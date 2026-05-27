---
slug: kosten-erfassen
title: Kosten erfassen
category: erste-schritte
order: 70
keywords: [kosten, ausgabe, eingangsrechnung, beleg, brutto, eür, kategorie, §13b, reverse-charge]
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
Im Formular gibst Du ein:

1. **Beleg-Datum** — das Datum auf der Eingangsrechnung.
2. **Bezahlt am** — der Tag, an dem Du bezahlt hast. Dieses Datum
   zählt fürs Finanzamt (Cash-Basis nach §11 EStG). Wenn Du noch
   nicht bezahlt hast, hake "noch nicht bezahlt" an — Klein.Buch
   nimmt die Position dann erst in die EÜR auf, wenn Du die
   Zahlung nachträgst.
3. **Lieferant** — entweder ein bestehender Kontakt oder ein
   neuer Lieferanten-Name als Freitext.
4. **Rechnungs-Nr. des Lieferanten** — optional.
5. **Kategorie (EÜR)** — aus der Auswahl-Liste, siehe unten.
6. **Beschreibung** — Pflichtfeld, kurze Bezeichnung der Ausgabe.
7. **Netto, Umsatzsteuer-Satz, Brutto** — Klein.Buch rechnet die
   Felder gegenseitig aus. Als §19-Kleinunternehmer trägst Du den
   USt-Satz "0 % (Kleinunternehmer / steuerfrei)" und nimmst den
   vollen Brutto-Betrag als Ausgabe (siehe "Warum Brutto" unten).
8. **Zahlungs-Konto** — über welches Konto die Ausgabe gelaufen
   ist (Bankkonto, Bargeld-Kasse, PayPal, Stripe, Sonstiges).
9. **§13b-Reverse-Charge** — Toggle für Eingangsrechnungen
   ausländischer Anbieter, die ihre Leistung ohne Umsatzsteuer
   stellen und die Steuerschuld auf Dich übertragen. Für §19-
   Kleinunternehmer fast nie zutreffend; lass den Toggle aus, wenn
   Du Dir unsicher bist.
10. **Beleg-Anhang** — PDF oder Bild der Eingangsrechnung.
11. **Notiz** — freier Text.

Klick auf "Kosten speichern". Klein.Buch schreibt die Position
sofort fest. Eine Korrektur geht nur noch über eine Storno-Buchung.

## Warum Brutto

Als §19-Kleinunternehmer darfst Du keine Vorsteuer abziehen. Das
heißt: Die Umsatzsteuer, die auf Deinen Eingangsrechnungen steht,
ist für Dich Teil der Ausgabe. Du buchst deshalb den vollen
Brutto-Betrag als Kosten.

Klein.Buch v1.0 ist für §19-Kleinunternehmer gebaut. Regelbesteuerte
Workflows (Vorsteuer-Abzug, Umsatzsteuer-Voranmeldung) sind nicht
voll abgebildet.

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
mit dem Stichtag als Zahlungs-Datum. Praktisch für stehende
Lastschriften, bei denen der Abzug pünktlich auf den Cent kommt.

**Erinnerung.** Klein.Buch legt am Stichtag nur einen Eintrag
unter "Hinweise" an. Du klickst dort auf "buchen", prüfst Betrag
und Zahlungs-Datum und speicherst. Sinnvoll, wenn der echte
Belegbetrag jeden Monat etwas abweicht.

Lege wiederkehrende Vorlagen über das Menü "Kosten" und
"Wiederkehrende Abos" an.

## E-Rechnung empfangen

Wenn Du eine XRechnung oder ZUGFeRD-Datei per E-Mail bekommst,
nutzt Du in der Kosten-Liste den Knopf "E-Rechnung importieren".
Klein.Buch liest die XML-Daten aus der Datei, füllt die Felder
automatisch aus und legt das Original im Archiv ab. Du musst nur
noch das Zahlungs-Datum eintragen und speichern.

Klein.Buch prüft die XRechnung mit dem KoSIT-Validator. Der
Validator ist beratend, nie blockierend — bei Fehlern warnt die App
und Du importierst trotzdem.

Wenn Du regelmäßig E-Rechnungen per Mail bekommst, gibt es einen
schnelleren Weg über einen überwachten Ordner. In Klein.Buch heißt
der "Rechnungs-Eingang". Du legst die Datei dort ab, Klein.Buch
übernimmt sie automatisch in die Kosten. Mehr dazu im Kapitel
"Eingangsrechnungen via Ordner".

## Privatbewegungen

Wenn Du Geld zwischen Deinem geschäftlichen und Deinem privaten
Konto verschiebst, ist das keine Kosten-Position. Es ist eine
Privatbewegung. Dafür gibt es im Hauptmenü einen eigenen Eintrag
"Privat-Geld". Privatbewegungen beeinflussen die EÜR nicht. Mehr
dazu im Kapitel "Privatbewegungen".

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
