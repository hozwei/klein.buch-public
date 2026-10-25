---
slug: geschaeftsjahr-abschluss
title: Geschäftsjahr-Abschluss
category: bedienen
order: 200
keywords: [geschäftsjahr, abschluss, jahres-lock, festschreibung, prüfungssicher]
---

# Geschäftsjahr-Abschluss

Der Geschäftsjahr-Abschluss ist das endgültige Festschreiben eines
kompletten Jahres. Nach dem Abschluss kannst Du im abgeschlossenen
Jahr nichts mehr ändern: keine neuen Belege nachtragen, keine
Zahlung erfassen, keine Anlage anlegen, keine AfA-Buchung
korrigieren.

## Wann Du abschließt

Spätestens vor der Übermittlung der Steuererklärung. Empfehlung:
abschließen, sobald Du den DATEV-Export an den Steuerberater
gesendet oder die ELSTER-Erklärung abgegeben hast. Damit ist
ausgeschlossen, dass nachträgliche Änderungen die EÜR von der
abgegebenen Erklärung abweichen lassen.

## Vorbereitung

Bevor Du abschließt, gehe diese Liste durch:

1. Alle Rechnungen des Jahres sind festgeschrieben (keine Entwürfe
   mehr offen). Offene Entwürfe blockieren den Abschluss nicht, weil sie
   nicht zur EÜR gehören. Nach dem Abschluss lassen sie sich aber nicht
   mehr festschreiben, sondern nur noch verwerfen (Kapitel "Rechnungen").
2. Alle bis dahin eingegangenen Zahlungen sind erfasst.
3. Alle Kosten des Jahres sind erfasst, einschließlich Beleg-
   Anhang.
4. Alle Anlagen sind angelegt und die AfA des Jahres ist erzeugt.
5. Die EÜR sieht plausibel aus, Du hast sie gegen Deine
   Konto-Auszüge grob abgeglichen.
6. Steuerberater-ZIP oder ELSTER-Ausfüllhilfe ist erzeugt.

Klein.Buch zeigt Dir vor dem Abschluss die EÜR-Eckwerte und eine
Liste der offenen Forderungen (Rechnungen, die im abzuschließenden
Jahr ausgestellt, aber noch nicht bezahlt sind). Diese Liste ist
ein Hinweis, kein Blocker. Du entscheidest selbst, ob Du trotzdem
abschließt. Offene Forderungen wandern als unbezahlt weiter und
tauchen erst in dem Jahr in der EÜR auf, in dem sie tatsächlich
bezahlt werden (Cash-Basis, siehe Kapitel "EÜR und Cash-Basis").

Der Abschluss selbst läuft nur, wenn das Geschäftsjahr bereits
abgelaufen ist (also frühestens am 1. Januar des Folgejahres) und
das Backup entsperrt ist. Diese beiden Punkte prüft Klein.Buch
hart und bricht andernfalls mit einer klaren Meldung ab.

## Abschluss durchführen

Klick im Hauptmenü auf **"Geschäftsjahr"**. Klein.Buch zeigt die
Liste der Geschäftsjahre. Klick in der Zeile des abzuschließenden
Jahres auf **"Abschließen"**. Klein.Buch zeigt die endgültige EÜR
und einen Bestätigungs-Dialog. Klick auf "Jetzt abschließen".

Klein.Buch schreibt das Jahr fest, erzeugt ein Auto-Backup mit dem
Auslöser `fiscal_year.close` (damit Du genau diesen Stand jederzeit
zurückspielen kannst) und sperrt alle Tabellen für das
abgeschlossene Geschäftsjahr.

Läuft beim Klick gerade eine Buchung (etwa ein Abo-Lauf oder eine
Zahlung in einem zweiten Browser-Tab), wartet der Abschluss, bis sie
fertig ist; sie steht dann vollständig im abgeschlossenen Jahr.
Umgekehrt wartet eine Buchung, die während des Abschlusses ankommt,
und wird danach erneut geprüft. In ein gerade abgeschlossenes Jahr
rutscht so nichts unbemerkt hinein.

Im Audit-Log steht der Abschluss mit den EÜR-Eckwerten, der Zahl der
festgeschriebenen Rechnungen, Stornos, Kosten und Privatbewegungen
und der Zahl der noch offenen Entwürfe.

## Wenn die EÜR später vom Abschluss abweicht

Beim Abschluss merkt sich Klein.Buch die Eckwerte der EÜR
(Einnahmen, Ausgaben, AfA, Überschuss) unveränderlich. Bei jeder
Anzeige und jedem Export eines abgeschlossenen Jahres rechnet es die
EÜR neu aus den einzelnen Buchungen und vergleicht. Weicht das
Ergebnis ab, hat sich entweder der Datenbestand des Jahres verändert
(zum Beispiel durch einen Eingriff direkt in die Datenbank) oder eine
neuere Version von Klein.Buch rechnet anders. Dann gilt:

1. Klein.Buch blockiert nichts. Exporte bleiben möglich.
2. Unter "Hinweise" erscheint einmal je Jahr der dringende Hinweis
   "EÜR weicht vom Jahresabschluss ab".
3. Jeder Export vermerkt die Abweichung im Audit-Log und in der Datei:
   im EÜR-PDF als Kasten, in der ELSTER-Ausfüllhilfe als
   Kommentarzeile, in der Einzelaufstellung als Begleitdatei und im
   Steuerberater-Paket am Anfang der `LIESMICH.txt`, jeweils mit beiden
   Werten. Der DATEV-Buchungsstapel bleibt im festen DATEV-Format.
4. Maßgeblich für die abgegebene Erklärung ist der festgeschriebene
   Stand. Klär die Ursache mit Deinem Steuerberater.

## Storno aus geschlossenem Jahr

Wenn nach dem Abschluss ein Fehler einer alten Rechnung auffällt,
darfst Du nicht in das geschlossene Jahr zurück. Stattdessen
erzeugst Du im laufenden Jahr einen Storno. Klein.Buch ordnet den
Storno korrekt: Ursprung im alten Jahr, EÜR-Effekt im laufenden
Jahr. Das ist gesetzlich der saubere Weg.

## Notfälle

Es gibt keinen "Abschluss rückgängig"-Knopf. Wenn Du irrtümlich
abgeschlossen hast und einen schwerwiegenden Fehler entdeckst,
musst Du das Backup vom Stand vor dem Abschluss zurückspielen.
Klein.Buch hat dieses Backup automatisch mit dem Auslöser
"fiscal_year.close" angelegt. Lies das Kapitel "Backup und
Wiederherstellen". Verlass Dich nicht auf diese Notfall-Lösung
als regulärem Arbeitsmittel; sie ist nur für echte Notfälle
gedacht.

---

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.10*
