---
slug: zahlung-erfassen
title: Zahlung erfassen
category: erste-schritte
order: 60
keywords: [zahlung, bezahlt, teilzahlung, teilbezahlt, zahlungskonto, cash-basis, eür, paid-at, überfällig]
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

Öffne die Rechnung über das Menü "Rechnungen" und klick oben auf der
Seite auf **"+ Zahlung"**. Nach dem Versand ist das der Hauptknopf;
vorher steht er neben "Senden" (auf dem Handy im Menü "Mehr").
Klein.Buch öffnet die Seite "Zahlung zu Rechnung RE-…". Die Karte
"Rechnung" zeigt Brutto, bisher bezahlten und offenen Betrag.

Trage das Zahlungsdatum ein (Klein.Buch schlägt heute vor; ein
Datum in der Zukunft akzeptiert die App nicht, weil §11 EStG erst
den tatsächlichen Geld-Eingang zählt). Wenn der Kunde den vollen
offenen Betrag bezahlt hat, lass die Checkbox "Kompletter offener
Betrag" angehakt; das Feld "Betrag" zeigt dann genau diesen Betrag.

Unter "Zahlungskonto" wählst Du, wo das Geld eingegangen ist (Bank,
Kasse, PayPal …); Klein.Buch schlägt Dein Standardkonto vor. Das Konto
bestimmt im DATEV-Buchungsstapel das Geldkonto der Buchung (Bank, Kasse
oder Geldtransit für Zahlungsdienste). Mit "Kein Konto zuordnen" bucht
Klein.Buch die Zahlung auf die Bank. Konten legst Du unter
"Einstellungen → Konten" an. Unter "Notiz (optional)" kannst Du zum
Beispiel den Verwendungszweck festhalten.

Klick auf **"Zahlung buchen"**, oben auf der Seite oder am Ende des
Formulars. Fehlt etwas oder passt etwas nicht, steht die Meldung am
Feld. Klein.Buch bestätigt mit "Zahlung gebucht.", markiert die
Rechnung je nach Restbetrag als "Bezahlt" oder "Teilbezahlt" und
zählt die Zahlung ab sofort in der EÜR.

## Teilzahlung

Wenn der Kunde nur einen Teil bezahlt, nimm das Häkchen bei
"Kompletter offener Betrag" heraus und trage den tatsächlich
erhaltenen Betrag in das Feld "Betrag" ein, zum Beispiel 250,00. Der
Betrag muss größer als 0 € sein und darf den offenen Betrag nicht
übersteigen. Klein.Buch merkt sich den Teilbetrag und das Datum, der
Rechnungs-Status springt auf "Teilbezahlt".

Sobald die zweite Zahlung kommt, klick wieder auf "+ Zahlung" und
trage Datum und Betrag der zweiten Zahlung ein. Klein.Buch summiert
die Beträge automatisch.

Für die EÜR gilt: Jede Teilzahlung zählt im Jahr ihres
Geld-Eingangs. Wenn die erste Hälfte im Dezember 2026 kommt und
die zweite im Januar 2027, dann landet die erste Hälfte im
EÜR-Jahr 2026 und die zweite im EÜR-Jahr 2027.

## Gebuchte Zahlungen

Die Detail-Ansicht der Rechnung zeigt alle gebuchten Zahlungen in der
Karte "Zahlungen" mit Zahldatum, Betrag, Konto und Notiz.

Eine gebuchte Zahlung lässt sich nicht ändern oder löschen; einen
Knopf dafür gibt es nicht. Prüf Betrag, Datum und Konto deshalb vor
"Zahlung buchen". Eine falsch gebuchte Zahlung korrigierst Du über
einen Storno der Rechnung (siehe Kapitel "Storno statt Löschung") und
eine neue Rechnung, auf der Du die Zahlung richtig erfasst.

Nach dem Geschäftsjahr-Abschluss nimmt Klein.Buch keine Zahlung mit
einem Datum in diesem Jahr mehr an. Eine Korrektur geht dann nur noch
über einen Storno im aktuellen Jahr.

## Mahnung

Klein.Buch hat noch keine Mahn-Funktion. Ist die Fälligkeit
überschritten und noch ein Betrag offen, zeigt Klein.Buch die
Rechnung als "Überfällig", in der Rechnungs-Liste und auf der
Seite "Start". Die Mahnung schreibst Du außerhalb der App; einen Vermerk
dazu hältst Du bei Bedarf im Feld "Notizen" des Kontakts fest, denn
eine ausgestellte Rechnung lässt sich nicht mehr ändern. Eine
ausgebliebene Zahlung führt automatisch dazu, dass die Rechnung in
der EÜR nicht als Einnahme zählt, solange sie nicht bezahlt ist.
Das ist eine Folge der Cash-Basis und kein Bug.

## Bezahlt-Hinweis auf dem PDF

Im Rechnungs-Entwurf kannst Du einen Bezahlt-Hinweis pflegen: Hak in
der Karte "Bezahlung" das Feld "Teil oder ganz bereits gezahlt" an
und trag den "Text des Hinweises auf dem Beleg" ein oder wähl einen
der Textvorschläge. Was Du dort einträgst (zum Beispiel "Bezahlt am
30.05.2026" oder "Restbetrag fällig zum 15.06.2026"), druckt
Klein.Buch auf das PDF. Das XRechnung-XML enthält diese Notiz nicht —
sie ist nur eine Anzeige für Deinen Beleg-Ordner und den Kunden. Eine
automatische Befüllung beim Bezahlen gibt es nicht.

Der Hinweis bucht nichts. Ist nach dem Ausstellen noch ein Betrag
offen, erinnert die Rechnung mit "Bezahlt-Hinweis ohne Buchung."
daran, die Zahlung über "+ Zahlung" zu erfassen.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
