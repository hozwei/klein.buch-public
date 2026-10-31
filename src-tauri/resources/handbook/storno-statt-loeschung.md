---
slug: storno-statt-loeschung
title: Storno statt Löschung
category: recht-und-steuern
order: 360
keywords: [storno, storno-beleg, löschen, korrektur, gegenrechnung, gobd, prüfung, verwerfen]
---

# Storno statt Löschung

Wenn Du eine festgeschriebene Rechnung korrigieren willst, gibt
es nur einen rechtssicheren Weg: Du erzeugst einen Storno-Beleg.
Löschen der Original-Rechnung ist verboten. Klein.Buch hat
absichtlich keinen Lösch-Knopf für festgeschriebene Belege.

## Warum nicht löschen

Eine festgeschriebene Rechnung ist Teil Deiner Buchführung. Sie
ist unveränderlich. Würdest Du sie löschen, wäre die Lücke in der
Nummern-Reihenfolge ein Beweis, dass Du nachträglich etwas
verändert hast. Das Finanzamt würde Deine ganze Buchführung
anzweifeln.

GoBD verlangt deshalb: Eine fehlerhafte Buchung wird durch eine
Gegen-Buchung neutralisiert, nicht durch Löschung.

## Was ein Storno tut

Wähl in der Detail-Ansicht der Rechnung im Menü "Mehr" den Eintrag
**"Stornieren"**. Klein.Buch öffnet die Seite "Rechnung RE-…
stornieren"; trag Storno-Datum und Grund ein (der Grund steht später
auf dem Storno-Beleg) und klick "Storno-Beleg erstellen". Eine
Rückfrage "Rechnung RE-… stornieren?" fasst die Folgen zusammen: Es
entsteht ein Storno-Beleg mit eigener Nummer, das lässt sich nicht
rückgängig machen, die Rechnung selbst bleibt erhalten. Erst
"Stornieren" in dieser Rückfrage legt den Beleg an; "Abbrechen"
ändert nichts. Klein.Buch erzeugt einen neuen Beleg mit:

1. Eigenem Datum (Tag des Stornos)
2. Eigener Storno-Nummer (Form "ST-JJJJ-NNNN") — Jahr und Geschäftsjahr
   richten sich nach dem Storno-Datum: Ein Storno vom Januar 2027 zu einer
   Rechnung vom Dezember 2026 heißt `ST-2027-…` und gehört ins Jahr 2027
3. Den Positionen der Original-Rechnung mit umgekehrtem Vorzeichen
4. Dem Verweis „Storno zur Rechnung <Nummer> vom <Datum>“ unter der
   Überschrift „Stornorechnung“ — auch in der E-Rechnung (XML)
5. Der Leitweg-ID der Original-Rechnung in der E-Rechnung, falls die
   Rechnung eine hatte (Rechnung an eine Behörde)
6. Dem Leistungszeitpunkt der Original-Rechnung: ihrem Leistungszeitraum
   bzw. Leistungsdatum, ohne beides ihrem Rechnungsdatum. Der Storno
   korrigiert die ursprüngliche Leistung (§ 31 Abs. 5 UStDV); das
   Storno-Datum ist das Datum des Belegs, nicht das einer Leistung

Die Karte "Original-Rechnung" auf der Storno-Seite zeigt Leistungsdatum
bzw. Leistungszeitraum und, falls vorhanden, die Leitweg-ID der
Rechnung, damit Du vor dem Stornieren siehst, was der Storno-Beleg
übernimmt.

Das Storno-Datum liegt zwischen dem Rechnungsdatum und heute. Ist die
Rechnung schon (ganz oder teilweise) bezahlt, frühestens am Tag des letzten
Zahlungseingangs: Klein.Buch führt die Erstattung am Storno-Datum in der
EÜR, und erstatten lässt sich erst, was zugeflossen ist (§ 11 EStG). Den
frühesten zulässigen Tag nennt Klein.Buch unter dem Feld "Storno-Datum".

Der Original-Beleg bleibt unverändert in der Datenbank. Beide
Belege sind sichtbar und werden gemeinsam in der EÜR
berücksichtigt. Nach dem Stornieren öffnet Klein.Buch den
Storno-Beleg; er nennt Nummer und Datum der Original-Rechnung mit
einem Link dorthin. Die Original-Rechnung verweist umgekehrt auf den
Storno-Beleg und steht in der Liste als "Storniert".

## Und Entwürfe?

Ein Entwurf ist noch kein Beleg und braucht keinen Storno. Weil er seine
Nummer aber schon beim Anlegen bekommt, löscht Klein.Buch auch ihn nicht:
Einen nicht benötigten Entwurf **verwirfst** Du (Menü "Mehr" → "Entwurf
verwerfen", Kapitel "Rechnungen").
Zeitpunkt und Grund stehen am Entwurf und im Protokoll, die Nummer
bleibt vergeben. Auch ein Storno-Entwurf, der beim Stornieren hängen
geblieben ist, lässt sich so verwerfen; das nächste "Stornieren" legt
dann einen neuen an.

Aus Datenbeständen der früheren Desktop-Fassung kann ein halbes
Storno-Paar stammen: Der Storno ist ausgestellt, die Original-Rechnung
steht aber noch als offen da (etwa nach einem Absturz mitten im
Stornieren). Klickst Du bei so einer Rechnung auf "Stornieren", legt
Klein.Buch keinen zweiten Storno an, sondern markiert die Rechnung als
durch den vorhandenen Storno storniert und vermerkt das im Protokoll. Die
EÜR ändert sich dadurch nicht; sie rechnet den Storno schon.

## EÜR-Wirkung

Maßgeblich ist die Cash-Basis (§11 EStG): in die EÜR fließen nur
tatsächliche Geld-Bewegungen. Daraus folgen drei Fälle:

**Original unbezahlt.** Weder die Original-Rechnung noch der Storno
tauchen in der EÜR auf — eine Einnahme gab es nie.

**Original bezahlt, Storno im selben Jahr.** Beide Belege heben
sich in der EÜR dieses Jahres genau auf. Der Netto-Effekt ist null.

**Original bezahlt, Storno in einem späteren Geschäftsjahr.** Die
Einnahme bleibt im Zahlungs-Jahr in der EÜR stehen (unveränderlich,
weil das Jahr ja schon abgeschlossen sein kann). Der Storno wirkt
im Jahr des Storno-Datums als negative Einnahme und schmälert dort
den Gewinn.

## Was Du dem Kunden schickst

Sende dem Kunden den Storno-Beleg: Auf dem Storno-Beleg ist "Senden"
der nächste Schritt, die Versand-Seite heißt "Storno-Beleg ST-…
versenden". Beilage: optional eine kurze
Erklärung, was korrigiert wurde. Wenn die Korrektur eine neue
korrekte Rechnung erfordert, sende auch die neue Rechnung. Drei
Dokumente beim Kunden: Original, Storno, Neue Rechnung. Das
schafft eine saubere Spur.

Wenn Du nur "Es war doch keine Leistung" feststellst, reicht der
Storno allein.

## Häufige Fehler, die einen Storno verlangen

1. Falsche Kunden-Adresse aufgedruckt.
2. Falscher Betrag, falsche Menge.
3. Falsches Leistungsdatum.
4. Aus Versehen Umsatzsteuer auf der Rechnung gestanden. In
   Klein.Buch ist das technisch nicht möglich, solange am
   Leistungsdatum die Kleinunternehmer-Regelung gilt: §14c UStG würde sonst dazu führen, dass
   Du die ausgewiesene Umsatzsteuer tatsächlich ans Finanzamt
   abführen musst, obwohl Du als Kleinunternehmer keine schuldest.
5. Die Leistung wurde gar nicht erbracht.

In all diesen Fällen: Storno-Beleg erstellen, gegebenenfalls
neue korrekte Rechnung schreiben, dem Kunden beides senden.

## Storno eines Stornos

Du kannst einen Storno-Beleg nicht stornieren. Wenn Du Dich beim
Stornieren vertan hast, schreibst Du eine neue Rechnung, die das
Verhalten korrigiert (zum Beispiel die Original-Rechnung erneut
schreiben mit dem korrekten Inhalt). Klein.Buch lässt zwei
Belege gleichen Inhalts ausdrücklich zu, damit Korrektur-Folge-
Korrekturen sauber dokumentiert sind.

## Versehentlich falscher Storno

Wenn Du eine Rechnung versehentlich storniert hast und das nicht
wolltest: Eine neue Rechnung mit denselben Daten anlegen und
versenden. Klein.Buch akzeptiert das. In Deiner Akte beim Kunden
liegen dann drei Belege: Original, Versehentlicher Storno, Neue
Rechnung. Vermerk dazu in den Notizen am Kontakt, damit Du beim
Erinnern weißt, was passiert ist.

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
