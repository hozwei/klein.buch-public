---
slug: geschaeftsjahr-abschluss
title: Geschäftsjahr-Abschluss
category: bedienen
order: 200
keywords: [geschäftsjahr, abschluss, jahres-lock, festschreibung, prüfungssicher, abschlussprotokoll, offene forderungen]
---

# Geschäftsjahr-Abschluss

Der Geschäftsjahr-Abschluss ist das endgültige Festschreiben eines
kompletten Jahres. Nach dem Abschluss kannst Du im abgeschlossenen
Jahr nichts mehr ändern: keine neuen Belege nachtragen, keine
Zahlung erfassen oder stornieren, keine Anschaffung anlegen, keine
AfA-Buchung korrigieren.

## Wann Du abschließt

Spätestens vor der Übermittlung der Steuererklärung. Empfehlung:
abschließen, sobald Du den DATEV-Export an den Steuerberater
gesendet oder die ELSTER-Erklärung abgegeben hast. Damit ist
ausgeschlossen, dass nachträgliche Änderungen die EÜR von der
abgegebenen Erklärung abweichen lassen.

## Vorbereitung

Bevor Du abschließt, gehe diese Liste durch:

1. Alle Rechnungen des Jahres sind ausgestellt (keine Entwürfe
   mehr offen). Offene Entwürfe blockieren den Abschluss nicht, weil sie
   nicht zur EÜR gehören. Nach dem Abschluss lassen sie sich aber nicht
   mehr ausstellen, sondern nur noch verwerfen (Kapitel "Rechnungen").
2. Alle bis dahin eingegangenen Zahlungen sind erfasst. Eine falsch
   erfasste Zahlung dieses Jahres stornierst Du vorher und erfasst sie
   richtig neu (Kapitel "Zahlung erfassen", Abschnitt "Falsch erfasste
   Zahlung stornieren"). Nach dem Abschluss lässt sie sich nicht mehr
   stornieren.
3. Alle Kosten des Jahres sind erfasst, einschließlich Beleg-
   Anhang.
4. Alle Anschaffungen sind angelegt und die AfA des Jahres ist erzeugt.
5. Die EÜR sieht plausibel aus, Du hast sie gegen Deine
   Konto-Auszüge grob abgeglichen.
6. Steuerberater-ZIP oder ELSTER-Ausfüllhilfe ist erzeugt.

Klein.Buch zeigt Dir vor dem Abschluss die EÜR-Eckwerte und in der
Karte "Offene Forderungen" alle ausgestellten Rechnungen, die noch nicht
(vollständig) bezahlt sind, mit Kunde, Datum, Fälligkeit und offenem
Betrag, aus allen Jahren. Diese Liste ist
ein Hinweis, kein Blocker. Du entscheidest selbst, ob Du trotzdem
abschließt. Offene Forderungen wandern als unbezahlt weiter und
tauchen erst in dem Jahr in der EÜR auf, in dem sie tatsächlich
bezahlt werden (Cash-Basis, siehe Kapitel "EÜR und Cash-Basis").

Der Abschluss selbst läuft nur, wenn das Geschäftsjahr bereits
abgelaufen ist (also frühestens am 1. Januar des Folgejahres) und
das Backup entsperrt ist. Diese beiden Punkte prüft Klein.Buch
hart und bricht andernfalls mit einer klaren Meldung ab.

## Abschluss durchführen

Klick im Hauptmenü auf **"Geschäftsjahr"**. Die Karte "Übersicht"
zeigt je Jahr den Status ("Offen" oder "Abgeschlossen" mit Datum),
Einnahmen, Ausgaben und Ergebnis. Beim laufenden Jahr steht "läuft
noch"; offene Entwürfe zeigt ein Zusatz wie "2 Entwürfe offen", fehlende
Abschreibungen ein Vermerk wie "1 AfA offen". Klick in der Zeile des
abzuschließenden Jahres auf **"Abschließen"**.

Die Rückfrage "Geschäftsjahr … abschließen?" nennt Einnahmen, Ausgaben,
darin Abschreibungen (AfA) und Ergebnis. Sind Rechnungs- oder
Angebots-Entwürfe mit Datum in diesem Jahr noch offen, warnt sie davor:
Nach dem Abschluss lässt sich in diesem Jahr nichts mehr festschreiben.
Fehlt noch AfA, sagt sie, dass Klein.Buch diese beim Abschluss
nachbucht. Klick auf "Jetzt abschließen"; "Abbrechen" ändert nichts.
Ist Deine letzte Bestätigung länger als fünf Minuten her, fragt
Klein.Buch danach im Dialog "Aktion bestätigen: Geschäftsjahr
abschließen" Dein Daten-Passwort und den App-Code ab. Zum Schluss meldet
es "Geschäftsjahr … abgeschlossen und festgeschrieben."

Ob die AfA zur Jahreswende automatisch gebucht wird, stellst Du oben auf
der Seite mit dem Schalter "Abschreibungen (AfA) zur Jahreswende
automatisch buchen" ein (empfohlen).

Klein.Buch schreibt das Jahr fest, erzeugt ein Auto-Backup (damit Du
genau diesen Stand jederzeit zurückspielen kannst; im "Backup-Protokoll"
steht es mit dem Auslöser "Beleg festgeschrieben") und sperrt alle
Tabellen für das abgeschlossene Geschäftsjahr.

Läuft beim Klick gerade eine Buchung (etwa ein Abo-Lauf oder eine
Zahlung in einem zweiten Browser-Tab), wartet der Abschluss, bis sie
fertig ist; sie steht dann vollständig im abgeschlossenen Jahr.
Umgekehrt wartet eine Buchung, die während des Abschlusses ankommt,
und wird danach erneut geprüft. In ein gerade abgeschlossenes Jahr
rutscht so nichts unbemerkt hinein.

Im Audit-Log steht der Abschluss mit den EÜR-Eckwerten, der Zahl der
festgeschriebenen Rechnungen, Stornos, Kosten und Privatbewegungen
und der Zahl der noch offenen Entwürfe.

## Abschlussprotokoll

Für abgeschlossene Jahre zeigt die Seite "Geschäftsjahr" unten die Karte
"Abschlussprotokoll": je Jahr den Zeitpunkt des Abschlusses und die
Eckwerte, wie sie beim Abschluss festgeschrieben wurden (Einnahmen,
Ausgaben, darin AfA, Ergebnis), dazu die Zahl der festgeschriebenen
Anschaffungen und AfA-Buchungen und die Version von Klein.Buch, mit der
abgeschlossen wurde.

## Wenn die EÜR später vom Abschluss abweicht

Beim Abschluss merkt sich Klein.Buch die Eckwerte der EÜR
(Einnahmen, Ausgaben, AfA, Überschuss) unveränderlich. Bei jeder
Anzeige und jedem Export eines abgeschlossenen Jahres rechnet es die
EÜR neu aus den einzelnen Buchungen und vergleicht. Weicht das
Ergebnis ab, hat sich entweder der Datenbestand des Jahres verändert
(zum Beispiel durch einen Eingriff direkt in die Datenbank) oder eine
neuere Version von Klein.Buch rechnet anders. Dann gilt:

1. Klein.Buch blockiert nichts. Exporte bleiben möglich.
2. Die Seite "Geschäftsjahr" markiert das Jahr in der Übersicht mit
   "Abweichung" und verweist auf das Abschlussprotokoll; die
   Steuer-Übersicht (EÜR) und die Seite "EÜR-Export" zeigen die Warnung
   "Abweichung vom Jahresabschluss." mit den betroffenen Werten und dem
   Ergebnis beim Abschluss und heute. Das Ansehen dieser Seiten schreibt
   nichts.
3. Sobald Du das Jahr exportierst, erscheint unter "Hinweise" einmal je
   Jahr der dringende Hinweis "EÜR weicht vom Jahresabschluss ab".
4. Jeder Export vermerkt die Abweichung im Audit-Log und in der Datei:
   im EÜR-PDF als Kasten, in der ELSTER-Ausfüllhilfe als
   Kommentarzeile, in der Einzelaufstellung als Begleitdatei und im
   Steuerberater-Paket am Anfang der `LIESMICH.txt`, jeweils mit beiden
   Werten. Der DATEV-Buchungsstapel bleibt im festen DATEV-Format.
5. Maßgeblich für die abgegebene Erklärung ist der festgeschriebene
   Stand. Klär die Ursache mit Deinem Steuerberater.

## Storno aus geschlossenem Jahr

Wenn nach dem Abschluss ein Fehler einer alten Rechnung auffällt,
darfst Du nicht in das geschlossene Jahr zurück. Stattdessen
erzeugst Du im laufenden Jahr einen Storno. Klein.Buch ordnet den
Storno korrekt: Ursprung im alten Jahr, EÜR-Effekt im laufenden
Jahr. Das ist gesetzlich der saubere Weg.

Anders bei einer falsch erfassten **Zahlung**: Ihre Gegenbuchung
gehört an das ursprüngliche Zahldatum, also in das abgeschlossene Jahr.
Deshalb lehnt Klein.Buch "Zahlung stornieren" dort ab. Ist die
Erklärung für dieses Jahr schon abgegeben und führt die falsche Zahlung
zu zu niedriger Steuer, ist die Erklärung zu berichtigen (§ 153 AO); zu
hoch erfasste Einnahmen lassen sich auf Antrag ändern (§ 173 Abs. 1
Nr. 2, § 164 Abs. 2 AO). Kläre das mit Deiner Steuerberatung. Ein Storno der Rechnung ist dafür nicht der
richtige Weg, weil er die Einnahme erst im laufenden Jahr ausgleicht.

## Notfälle

Es gibt keinen "Abschluss rückgängig"-Knopf. Wenn Du irrtümlich
abgeschlossen hast und einen schwerwiegenden Fehler entdeckst,
musst Du ein Backup vom Stand vor dem Abschluss zurückspielen, also
eines, das laut "Backup-Protokoll" vor dem Zeitpunkt des Abschlusses
entstanden ist. Das Auto-Backup, das Klein.Buch beim Abschluss anlegt,
enthält das Jahr bereits abgeschlossen. Lies das Kapitel "Backup und
Wiederherstellen". Verlass Dich nicht auf diese Notfall-Lösung
als regulärem Arbeitsmittel; sie ist nur für echte Notfälle
gedacht.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
