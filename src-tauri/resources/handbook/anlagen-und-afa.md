---
slug: anlagen-und-afa
title: Anschaffungen und AfA
category: bedienen
order: 170
keywords: [anschaffung, anlage, afa, abschreibung, nutzungsdauer, anlagevermögen, gwg, computer]
---

# Anschaffungen und AfA

Eine Anschaffung ist ein geschäftlich genutzter Gegenstand, den Du
über die Buchhaltung erfasst. Wenn die Anschaffung dauerhaft im
Betrieb genutzt wird (länger als ein Jahr) und einen bestimmten
Wert übersteigt, gehört sie ins Anlagenverzeichnis und Du
schreibst sie über die Nutzungsdauer ab. Die jährliche Verteilung
heißt **AfA** (Absetzung für Abnutzung). Klein.Buch verwaltet Dir
das Anlagenverzeichnis und führt die AfA automatisch.

In der App heißt der Bereich **"Anschaffungen"** (Hauptmenü).

## Wann eine Anschaffung eine Anlage ist

Stand 2026: Bei Anschaffungen über 800 € netto musst Du den
Aufwand über die Nutzungsdauer verteilt absetzen — sie sind klare
Anlagen. Unter 800 € netto darfst Du den vollen Betrag sofort als
Ausgabe absetzen (geringwertiges Wirtschaftsgut, GWG, §6 Abs. 2
EStG). Du gibst in Klein.Buch immer den Netto-Wert ein; als
§19-Kleinunternehmer ist Netto gleich Brutto, weil Du keine
Vorsteuer abziehst — die 800 €-Grenze gilt für Dich also auf den
vollen Kaufpreis.

Typische Anlagen sind Notebook, Werkstattmaschine, Geschäftswagen,
Büromöbel ab einem gewissen Preis.

Bei Unsicherheit, ob eine konkrete Anschaffung eine Anlage ist
oder sofort absetzbar, frag Deinen Steuerberater.

## Anschaffung anlegen

Klick im Hauptmenü auf "Anschaffungen" und dann auf **"+ Neue
Anschaffung"**. Trage ein:

1. **Bezeichnung** (zum Beispiel "Notebook ThinkPad X1")
2. **Anschaffungs-Datum**
3. **Anschaffungskosten netto** (€) — als §19-Kleinunternehmer ist
   das gleich dem Brutto-Wert
4. **AfA-Kategorie** — Vorschlag aus der amtlichen Tabelle des
   Bundesfinanzministeriums
5. **Nutzungsdauer** in Jahren (Vorschlag aus der AfA-Kategorie)
6. **AfA-Methode**, siehe nächster Abschnitt
7. **Betrieblicher Anteil** (0–100 %) — bei Misch-Nutzung
   (z. B. Kfz mit privatem Anteil) gibst Du hier den
   geschäftlichen Anteil ein. Klein.Buch rechnet die AfA-Basis
   entsprechend
8. **Lieferant** — optional ein bestehender Kontakt
9. **Notiz** — optional

Speichere. Klein.Buch berechnet die Jahres-AfA und legt die
Buchungen für jedes Jahr der Nutzungsdauer als Vormerkung an.

## AfA-Methoden

Klein.Buch v1.0 unterstützt drei AfA-Methoden:

**Lineare Abschreibung** (Standard). Anschaffungskosten geteilt
durch Nutzungsdauer, jedes Jahr derselbe Betrag.

**Sofortabschreibung (geringwertig, ≤ 800 € netto)**. Der volle
Betrag wird im Kaufjahr als Ausgabe gebucht.

**Computer/Software (1 Jahr, faktisch sofort)**. Sonderregel nach
BMF-Schreiben vom 22.02.2022 für Computer-Hardware und Software.
Die Anschaffung wird auf ein Jahr abgeschrieben — wirtschaftlich
ist das eine Sofortabschreibung.

## AfA-Lauf

Bei eingeschaltetem Automatik-Schalter (Standard, in "Geschäftsjahr"
einstellbar) erzeugt Klein.Buch die AfA-Buchungen zum 1. Januar
jedes Geschäftsjahres automatisch. Du siehst die Buchung in der
Anschaffungs-Detail-Ansicht und in der EÜR als Ausgabe.

Wenn der automatische Lauf abgeschaltet ist, klickst Du auf der
Anschaffungen-Liste auf **"Abschreibung buchen"** in der eigenen
Karte unterhalb der Tabelle. Tu das spätestens vor dem
Geschäftsjahr-Abschluss.

## Anschaffung verkaufen oder verschrotten

Wenn Du eine Anschaffung vor Ablauf der Nutzungsdauer ausscheidest
(Verkauf, Verschrottung, Diebstahl), öffne die Detail-Ansicht und
klick auf **"Veräußern / Entsorgen"**. Trage Datum, Art (Verkauf
oder Verschrottung) und gegebenenfalls den Veräußerungs-Erlös ein.
Bestätige mit "Veräußerung bestätigen". Klein.Buch beendet die
laufende AfA, verbucht den Rest-Buchwert als Ausgabe und legt den
Verkaufs-Erlös als Einnahme an.

Bei einem Verkauf an einen anderen Unternehmer brauchst Du
zusätzlich eine Verkaufsrechnung. Die legst Du normal im Bereich
"Rechnungen" an.

## Korrektur

Vor dem ersten AfA-Lauf kannst Du die Anschaffungs-Daten frei
korrigieren. Sobald die erste AfA-Buchung steht, sind die
Kern-Felder gesperrt. Wenn Du dennoch korrigieren musst, gibt es
in der Detail-Ansicht den Knopf **"Abschreibung zurücksetzen"** —
das funktioniert nur, solange das Geschäftsjahr nicht abgeschlossen
ist. Nach dem Geschäftsjahr-Abschluss sind die AfA-Werte für das
abgeschlossene Jahr endgültig fest.

## AfA in der EÜR

Die jährliche AfA-Buchung erscheint in der EÜR als Ausgabe und
schmälert den Gewinn. Sie ist eine reine Buchungs-Ausgabe und nicht
mit einem Geldfluss verbunden, weil Du die Anschaffung ja schon
im Kaufjahr bezahlt hast. Diese Trennung ist eine Ausnahme von der
reinen Cash-Basis und steht ausdrücklich in §4 Abs. 3 Satz 3 EStG.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
