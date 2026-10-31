---
slug: zahlung-erfassen
title: Zahlung erfassen
category: erste-schritte
order: 60
keywords: [zahlung, bezahlt, teilzahlung, teilbezahlt, zahlungskonto, cash-basis, eür, paid-at, überfällig, gegenbuchung, zahlung stornieren, fehlbuchung]
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

Die Detail-Ansicht der Rechnung zeigt alle Buchungen in der Karte
"Zahlungen": Zahldatum, Art ("Zahlung" oder "Gegenbuchung"), Betrag,
Konto und Notiz bzw. Grund. Eine stornierte Zahlung bleibt in der
Liste stehen und trägt den Vermerk "Storniert" mit dem Zeitpunkt der
Gegenbuchung; die Gegenbuchung nennt die Zahlung, die sie storniert,
und ihren Grund. Die Karte bleibt auch dann sichtbar, wenn nach einer
Gegenbuchung nichts mehr bezahlt ist.

Eine gebuchte Zahlung wird nie geändert oder gelöscht. Klein.Buch
führt die Zahlungen in einem Zahlungsbuch, in dem nichts überschrieben
wird. Eine falsch erfasste Zahlung korrigierst Du mit einer
Gegenbuchung (nächster Abschnitt) und erfasst sie danach richtig neu.
Prüf Betrag, Datum und Konto trotzdem vor "Zahlung buchen" — jede
Korrektur bleibt als Paar sichtbar.

Nach dem Geschäftsjahr-Abschluss nimmt Klein.Buch keine Zahlung mit
einem Datum in diesem Jahr mehr an.

## Falsch erfasste Zahlung stornieren

Hast Du eine Zahlung falsch erfasst (falscher Betrag, falsches Datum,
falsches Konto, falsche Rechnung oder das Geld kam gar nicht), stornierst
Du sie mit einer Gegenbuchung: derselbe Betrag mit umgekehrtem
Vorzeichen, mit Datum und Konto der falschen Zahlung. Beide Buchungen
bleiben sichtbar, in der Rechnung, in der EÜR-Einzelaufstellung und im
DATEV-Stapel.

1. Öffne die Rechnung. In der Karte "Zahlungen" klickst Du in der Zeile
   der falschen Zahlung auf **"Mehr"** und wählst **"Zahlung
   stornieren …"**.
2. Der Dialog "Zahlung stornieren?" nennt Rechnung, Betrag, Zahldatum
   und Konto der Zahlung. Trag unter **"Grund"** ein, was falsch war,
   zum Beispiel "Betrag falsch erfasst, laut Kontoauszug 450,00 €". Der
   Grund ist Pflicht (mindestens 10 Zeichen) und steht an der
   Gegenbuchung und im Protokoll. Ist er zu kurz, steht die Meldung am
   Feld.
3. Klick auf **"Zahlung stornieren"**. Liegt Deine letzte Bestätigung
   länger als fünf Minuten zurück, fragt Klein.Buch Daten-Passwort und
   App-Code ab, wie bei anderen unumkehrbaren Schritten.
4. Klein.Buch bestätigt mit "Zahlung vom … über … storniert." und bietet
   **"Zahlung neu erfassen"** an. Der Knopf öffnet die Seite "+ Zahlung"
   mit Datum, Konto und Notiz der stornierten Zahlung als Vorschlag;
   prüf sie und trag Betrag und Datum laut Kontoauszug ein. Kam das
   Geld gar nicht, erfasst Du nichts neu.

- **Wirkung:** Die Gegenbuchung wirkt am ursprünglichen Zahldatum. Eine
  Zahlung, die so nie geflossen ist, war keine Einnahme (§ 11 EStG). Die
  EÜR des Jahres der falschen Zahlung ändert sich also. Hast Du die
  Auswertungen dieses Jahres schon an Deine Steuerberatung gegeben, gib
  ihr die neuen.
- **Status:** Die Rechnung geht zurück auf "Teilbezahlt" oder, ohne
  verbleibende Zahlung, auf "Versendet" bzw. "Ausgestellt". Ist sie
  fällig, zeigt Klein.Buch sie wieder als "Überfällig".

Nicht möglich ist das Stornieren einer Zahlung — der Eintrag im Menü
ist dann gesperrt und nennt den Grund:

- wenn das Geschäftsjahr des Zahldatums abgeschlossen ist. Ist die
  Erklärung für dieses Jahr schon abgegeben und führt die falsche
  Zahlung zu zu niedriger Steuer, ist die Erklärung zu berichtigen
  (§ 153 AO); zu hoch erfasste Einnahmen lassen sich auf Antrag ändern
  (§ 173 Abs. 1 Nr. 2, § 164 Abs. 2 AO). Kläre das mit Deiner
  Steuerberatung;
- bei einer stornierten Rechnung, einem Storno-Beleg oder solange es zur
  Rechnung einen offenen Storno-Entwurf gibt. Willst Du eine Rechnung
  stornieren und ist eine ihrer Zahlungen falsch erfasst, storniere
  deshalb zuerst die Zahlung, dann die Rechnung.

Eine Zahlung, die schon storniert ist, und eine Gegenbuchung haben kein
Menü.

Lehnt Klein.Buch das Stornieren ab (zum Beispiel, weil das Jahr
inzwischen abgeschlossen wurde), steht der Grund im Dialog; es ist dann
nichts gebucht.

**Abgrenzung:** "Zahlung stornieren" ist nur für Fehlbuchungen da. Hast
Du dem Kunden Geld zurückgezahlt, ist das kein Fehler in der Buchung,
sondern ein Abfluss am Tag der Rückzahlung. Den bildest Du über den
Storno der Rechnung ab (siehe Kapitel "Storno statt Löschung").

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

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
