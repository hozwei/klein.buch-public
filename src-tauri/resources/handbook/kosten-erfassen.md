---
slug: kosten-erfassen
title: Kosten erfassen
category: erste-schritte
order: 70
keywords: [kosten, ausgabe, eingangsrechnung, beleg, brutto, eür, kategorie, §13b, reverse-charge, festschreiben, foto]
---

# Kosten erfassen

Kosten sind alle geschäftlichen Ausgaben, zum Beispiel Material,
Software-Abos, Werkzeug, Büro-Miete, Fortbildungen, Tankquittungen.
Du trägst sie in Klein.Buch ein, sobald Du die Eingangsrechnung
erhalten und bezahlt hast. Klein.Buch zieht sie dann in der EÜR
als Ausgabe ab.

## Belege sammeln

Hebe jeden geschäftlichen Beleg auf. Eine Eingangsrechnung ohne
Beleg erkennt das Finanzamt nicht an. Klein.Buch erlaubt Dir, zu
jeder Kostenposition den Beleg als PDF oder Bild anzuhängen. Der
Beleg landet im Archiv und ist 10 Jahre nachweisbar.

## Schritte

Klick im Hauptmenü auf "Kosten" und dann auf **"+ Neue Kosten"**.
Es öffnet sich die Seite "Neue Kosten erfassen". Felder mit Stern sind
Pflicht. Im Formular gibst Du ein:

1. **Beleg (PDF oder Bild, optional)** — die Eingangsrechnung. Das
   Feld steht ganz oben, damit Du erst den Beleg wählst und dann
   abtippst. Auf Geräten mit Touch-Bildschirm öffnet der Knopf "Foto
   aufnehmen" daneben direkt die Kamera.
2. **Beleg-Datum** — das Datum auf der Eingangsrechnung.
3. **Bezahlt am** — der Tag, an dem Du bezahlt hast. Dieses Datum
   zählt fürs Finanzamt (Cash-Basis nach §11 EStG). Wenn Du noch
   nicht bezahlt hast, hake "noch nicht bezahlt" an — Klein.Buch
   nimmt die Position dann erst in die EÜR auf, wenn Du die
   Zahlung nachträgst. Liegt das Zahldatum vor dem Beleg-Datum (etwa
   bei Vorkasse), weist das Formular darauf hin; erlaubt ist es.
4. **Lieferant (Kontakt)** und **Lieferanten-Name** — wählst Du einen
   bestehenden Kontakt, trägt Klein.Buch dessen Namen ein. Ohne Kontakt
   schreibst Du den Namen als Freitext; der Lieferanten-Name ist
   Pflicht.
5. **Rechnungs-Nr. des Lieferanten** — optional.
6. **Kategorie (EÜR)** — aus der Auswahl-Liste, siehe unten.
7. **Beschreibung** — Pflichtfeld, kurze Bezeichnung der Ausgabe.
8. **Netto, USt-Satz, USt-Betrag** — übernimm Netto und Satz vom
   Beleg; Klein.Buch rechnet den USt-Betrag und den Brutto-Betrag
   aus. Rundet der Lieferant anders, wähle beim Satz "individuell
   (Betrag vom Beleg)" und trag den USt-Betrag genau vom Beleg ein.
   Steht auf dem Beleg keine Umsatzsteuer (etwa weil der Lieferant
   selbst Kleinunternehmer ist), wähle "0 % (steuerfrei / §13b /
   Kleinunternehmer)". Als §19-Kleinunternehmer zählt in jedem Fall
   der volle Brutto-Betrag als Ausgabe (siehe "Warum Brutto" unten).
   Beträge tippst Du wie gewohnt, etwa "1.234,56"; ein Minuszeichen
   gibt es nicht.
9. **Zahlungs-Konto** — über welches Konto die Ausgabe gelaufen
   ist, zum Beispiel Bankkonto oder Bargeld-Kasse. Zur Auswahl stehen
   Deine Konten aus "Einstellungen" und "Konten"; vorbelegt ist Dein
   Standard-Konto.
10. **§13b Reverse-Charge (Steuerschuldnerschaft des
    Leistungsempfängers)** — Haken für Eingangsrechnungen, bei
    denen nicht der Lieferant, sondern Du die Umsatzsteuer schuldest.
    Das betrifft auch Kleinunternehmer, zum Beispiel Online-Werbung
    bei Anbietern mit Sitz im EU-Ausland oder Software-Abos aus dem
    Ausland. Die Rechnung kommt dann ohne Umsatzsteuer; Du trägst
    Netto = Brutto und eine Umsatzsteuer von 0 ein. Mit dem Haken
    stellt das Formular den USt-Satz auf 0 % und sperrt die übrigen
    Sätze und den USt-Betrag (0); ein Hinweis darunter erklärt das.
    Kommt trotzdem ein Umsatzsteuer-Betrag an, meldet Klein.Buch
    den Fehler am Feld "USt-Betrag".
    Was Du sonst noch tun musst, steht unten unter "§13b: Du schuldest die Steuer".
11. **Notiz (optional)** — freier Text.

Klick auf "Kosten festschreiben", oben auf der Seite oder am Ende des
Formulars. Über dem unteren Knopf steht, was das bedeutet: "Danach
nicht mehr änderbar — Korrektur nur per Storno." Fehlt eine Angabe,
steht die Meldung am Feld, und der Fokus springt zum ersten fehlerhaften
Feld. Sonst fragt Klein.Buch nach und nennt die Eckwerte: Belegdatum,
Lieferant, Brutto, Zahlung und Beleg. Mit "Festschreiben" schreibt
Klein.Buch die Position fest und öffnet ihre Seite; "Abbrechen" ändert
nichts. Eine Korrektur geht danach nur noch über einen Storno;
"Korrigiert neu erfassen" übernimmt dabei die Werte (Kapitel "Kosten").

## Warum Brutto

Als §19-Kleinunternehmer darfst Du keine Vorsteuer abziehen. Das
heißt: Die Umsatzsteuer, die auf Deinen Eingangsrechnungen steht,
ist für Dich Teil der Ausgabe. Du buchst deshalb den vollen
Brutto-Betrag als Kosten.

Klein.Buch v1.0 ist für §19-Kleinunternehmer gebaut. Regelbesteuerte
Workflows (Vorsteuer-Abzug, Umsatzsteuer-Voranmeldung) sind nicht
voll abgebildet.

Wechselt Dein Steuerstatus (Verzicht auf §19, Wegfall, Rückkehr), zählt
für jede Kostenposition Dein Steuerstatus bei Leistungsbezug, nicht der
im Jahr der Zahlung (§15 Abs. 1 UStG). Klein.Buch nimmt dafür das
Belegdatum, tagesgenau: Bei einem Wegfall am 15.07. bleiben Kosten vom
14.07. ohne Vorsteuer, Kosten vom 15.07. haben sie. Eine Rechnung
vom Dezember, als Du noch Kleinunternehmer warst, bleibt brutto ohne
Vorsteuer, auch wenn Du sie erst im Januar unter Regelbesteuerung
bezahlst. In welchem Jahr die Ausgabe in der EÜR steht, bestimmt
weiterhin das Zahldatum (§11 EStG). Die EÜR-Ausfüllhilfe nennt
Kosten, bei denen Leistungsbezug und Jahr der Zahlung unter
verschiedenem Steuerstatus liegen.

## §13b: Du schuldest die Steuer

Bei manchen Leistungen schuldet der Empfänger die Umsatzsteuer statt
des Lieferanten ("Reverse-Charge", §13b UStG). Typisch sind
Leistungen von Unternehmen mit Sitz im Ausland, etwa Werbeanzeigen,
Cloud-Dienste oder Software-Lizenzen aus dem EU-Ausland. Die
Kleinunternehmer-Regelung hilft hier nicht: Auch als
Kleinunternehmer schuldest Du diese Steuer (§13b Abs. 5 und 8
UStG). Auf der Rechnung steht dann meist "Steuerschuldnerschaft des
Leistungsempfängers" oder "Reverse Charge".

Was Du tun musst:

1. In Klein.Buch die Rechnung netto erfassen (Umsatzsteuer 0) und
   den Haken bei "§13b Reverse-Charge" setzen.
2. Für den Monat, in dem die Steuer entsteht (in der Regel der Monat
   der Leistung, spätestens der Folgemonat), eine
   Umsatzsteuer-Voranmeldung über ELSTER abgeben und die Steuer
   zahlen (§18 Abs. 4a UStG). Klein.Buch berechnet diese Steuer
   nicht und erstellt keine Voranmeldung.
3. Vorsteuer darfst Du als Kleinunternehmer nicht abziehen. Die an
   das Finanzamt gezahlte Umsatzsteuer ist deshalb im Jahr der
   Zahlung eine Betriebsausgabe. Trag die Zahlung ans Finanzamt als
   eigene Kosten ein oder lass sie Deinen Steuerberater in der EÜR
   ergänzen.
4. Ist der Kauf eine Anlage (z. B. ein Gerät oder eine Lizenz, die Du
   über mehrere Jahre abschreibst), gehört die selbst geschuldete
   Steuer zu den Anschaffungskosten (§9b Abs. 1 EStG) und ist keine
   sofortige Betriebsausgabe. Übernimm die Rechnung zum
   Rechnungsbetrag als Anlage. Die Steuer erfasst Du bei der Zahlung
   ans Finanzamt als eigene Kosten (nur der Anteil dieser Anlage,
   Umsatzsteuer 0, ohne §13b-Schalter) und übernimmst sie als eigene
   Anlage mit gleichem Anschaffungsdatum, gleicher Methode und
   Nutzungsdauer. So wird sie genau einmal abgeschrieben. Dafür gibt
   es auf der Seite der Anlage im Abschnitt "Selbst geschuldete Steuer
   (§13b)" den Knopf "Steuer als Kosten erfassen". Er öffnet das
   Formular vorbelegt (Lieferant "Finanzamt", Kategorie "Sonstiges",
   USt-Satz 0 %, ohne §13b-Haken); nach dem Festschreiben geht es
   direkt zur Übernahme als Anlage. Mehr unter "Anschaffungen und AfA",
   Abschnitt "Aus Kosten aktivieren".

Wenn Du unsicher bist, ob eine Rechnung unter §13b fällt, frag
Deinen Steuerberater. Eine E-Rechnung, die neben §13b-Positionen
auch Umsatzsteuer ausweist, übernimmt Klein.Buch ohne den §13b-Haken
und mit einem Hinweis in der Notiz.

## Kategorien

Klein.Buch hat eine feste Liste von 14 Kosten-Kategorien:

1. Bürobedarf
2. Software / Lizenzen
3. Hardware
4. Reisekosten
5. Fremdleistungen
6. Wareneinkauf
7. Telefon / Internet
8. Kfz / Fahrzeug
9. Miete / Raumkosten
10. Versicherungen / Beiträge
11. Fortbildung
12. Gebühren / Bankspesen
13. Werbung / Marketing
14. Sonstiges

Jede Kategorie ist einem DATEV-Konto zugeordnet, damit der
DATEV-Export im Steuerberater-Paket das passende Konto trifft.
Wenn Du unsicher bist, wo eine Ausgabe hingehört, wähle "Sonstiges"
und frag Deinen Steuerberater beim nächsten Termin.

## Wiederkehrende Kosten

Manche Kosten kommen jeden Monat oder jedes Jahr (Miete,
Versicherungen, Domain-Gebühren, Software-Abos). Du musst sie nicht
jedes Mal von Hand eintragen. Klein.Buch hat dafür **Wiederkehrende
Abos** mit zwei Modi:

**Automatisch.** Klein.Buch bucht die Position am Stichtag selbst
mit dem Stichtag als Beleg-Datum, zunächst "noch nicht bezahlt". Die
Zahlung bestätigst Du, wenn die Abbuchung auf dem Konto steht.
Praktisch für stehende Lastschriften, bei denen der Abzug pünktlich
auf den Cent kommt.

**Erinnerung.** Klein.Buch legt am Stichtag nur einen Eintrag
"Abo fällig" unter "Hinweise" an. "Öffnen" beim Hinweis führt zur
Abo-Liste; dort legt "Buchen" die Position mit dem erwarteten Betrag
an und schreibt sie fest. Du prüfst Betrag und Datum und trägst die
Zahlung nach, sobald sie abgebucht ist.

Lege wiederkehrende Vorlagen über das Menü "Kosten" und
"Kosten-Abos" an. Mehr dazu im Kapitel "Kosten".

## E-Rechnung empfangen

Wenn Du eine XRechnung oder ZUGFeRD-Datei per E-Mail bekommst,
nutzt Du in der Kosten-Liste den Knopf "E-Rechnung importieren" (auf
dem Handy im Menü "Mehr") und lädst die Datei im Browser hoch.
Klein.Buch liest die XML-Daten aus der Datei, füllt die Felder
automatisch aus und legt das Original im Archiv ab. Du musst nur noch
Zahlungs-Datum und Kategorie prüfen und mit "Kosten festschreiben"
abschließen.

Klein.Buch prüft die XRechnung mit dem KoSIT-Validator. Der
Validator ist beratend, nie blockierend: Bei Fehlern warnt die App
und Du importierst trotzdem. Mehr dazu im Kapitel "Rechnungseingang
per Upload".

## Privatbewegungen

Wenn Du Geld zwischen Deinem geschäftlichen und Deinem privaten
Konto verschiebst, ist das keine Kosten-Position. Es ist eine
Privatbewegung. Dafür gibt es im Hauptmenü einen eigenen Eintrag
"Privatbewegungen". Privatbewegungen beeinflussen die EÜR nicht. Mehr
dazu im Kapitel "Privatbewegungen".

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
