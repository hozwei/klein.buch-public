---
slug: zahlung-erfassen
title: Zahlung erfassen
category: erste-schritte
order: 60
keywords: [zahlung, bezahlt, teilzahlung, cash-basis, eür, paid-at]
---

# Zahlung erfassen

Solange eine Rechnung noch nicht bezahlt ist, gilt sie für die EÜR
nicht als Einnahme. Klein.Buch nutzt die Cash-Basis. Das heißt:
Eine Einnahme zählt erst, wenn das Geld bei Dir eingegangen ist.

Du markierst eine Rechnung deshalb erst nach dem Geld-Eingang als
"bezahlt". Klein.Buch übernimmt diesen Tag dann als Zahlungs-Datum
in die EÜR-Berechnung.

## Wann Du eine Zahlung erfasst

Sobald das Geld auf Deinem Konto angekommen ist. Maßgeblich ist
das Datum auf dem Konto-Auszug, nicht das Datum, an dem der Kunde
die Überweisung angestoßen hat.

Bei Bar-Zahlung gilt der Tag der Übergabe.

## Schritte

Öffne die Rechnung über das Menü "Rechnungen". Klick die
betroffene Rechnung an und dann in der PageBar oben rechts auf
**"+ Zahlung"**. Klein.Buch öffnet eine eigene Zahlungs-Seite.

Trage das Zahlungs-Datum ein (Klein.Buch schlägt heute vor; ein
Datum in der Zukunft akzeptiert die App nicht, weil §11 EStG erst
den tatsächlichen Geld-Eingang zählt). Wenn der Kunde den vollen
Rechnungsbetrag bezahlt hat, lass die Checkbox "Komplett bezahlt"
angehakt und bestätige.

Klein.Buch markiert die Rechnung je nach Restbetrag als "Bezahlt"
oder "Teilzahlung", aktualisiert die EÜR im Hintergrund und legt
ein Auto-Backup an.

## Teilzahlung

Wenn der Kunde nur einen Teil bezahlt, hake die Checkbox "Komplett
bezahlt" ab und trage den tatsächlich erhaltenen Betrag in das
Betrag-Feld ein. Klein.Buch merkt sich den Teilbetrag und das
Datum, der Rechnungs-Status springt auf "Teilzahlung".

Sobald die zweite Zahlung kommt, klick wieder auf "+ Zahlung" und
trage Datum und Betrag der zweiten Zahlung ein. Klein.Buch summiert
die Beträge automatisch.

Für die EÜR gilt: Jede Teilzahlung zählt im Jahr ihres
Geld-Eingangs. Wenn die erste Hälfte im Dezember 2026 kommt und
die zweite im Januar 2027, dann landet die erste Hälfte im
EÜR-Jahr 2026 und die zweite im EÜR-Jahr 2027.

## Zahlung rückgängig machen

Solange das Geschäftsjahr noch nicht abgeschlossen ist, kannst Du
eine Zahlung wieder löschen, wenn Du Dich vertan hast. Klick im
Rechnungs-Fenster auf das X neben der erfassten Zahlung und
bestätige.

Nach dem Geschäftsjahr-Abschluss ist auch das Zahlungs-Feld
gesperrt. Eine Korrektur geht dann nur noch über eine Storno-
Buchung des aktuellen Jahres.

## Mahnung

Klein.Buch in Version 1.0 hat noch keine Mahn-Funktion. Wenn ein
Kunde nicht bezahlt, schreibst Du die Mahnung außerhalb der App
und hinterlegst gegebenenfalls eine Notiz an der Rechnung. Eine
ausgebliebene Zahlung führt automatisch dazu, dass die Rechnung in
der EÜR nicht als Einnahme zählt, solange sie nicht bezahlt ist.
Das ist eine Folge der Cash-Basis und kein Bug.

## Bezahlt-Hinweis auf dem PDF

An der Rechnung selbst kannst Du ein Freitext-Feld "Bezahlt-Hinweis"
pflegen. Was Du dort einträgst (zum Beispiel "Bezahlt am 30.05.2026"
oder "Restbetrag fällig zum 15.06.2026"), druckt Klein.Buch auf das
PDF. Das XRechnung-XML enthält diese Notiz nicht — sie ist nur eine
Anzeige für Deinen Beleg-Ordner und den Kunden. Eine automatische
Befüllung beim Bezahlen gibt es nicht.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
