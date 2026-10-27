---
slug: anlagen-und-afa
title: Anschaffungen und AfA
category: bedienen
order: 170
keywords: [anschaffung, anlage, afa, abschreibung, nutzungsdauer, anlagevermögen, gwg, computer, abgang, restbuchwert, §13b]
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
Anlagen. Bis 800 € netto darfst Du den vollen Betrag sofort als
Ausgabe absetzen (geringwertiges Wirtschaftsgut, GWG, §6 Abs. 2
EStG). Die 800 €-Grenze gilt immer für den Preis **ohne**
Umsatzsteuer — auch als §19-Kleinunternehmer, obwohl Du die
Umsatzsteuer nicht als Vorsteuer abziehst (R 9b Abs. 2 EStR). Ein
Notebook für 900 € brutto (756,30 € netto) ist also ein GWG.

Abgeschrieben wird dagegen der Betrag, den Dich die Anschaffung
kostet (§ 9b Abs. 1 EStG): bei Regelbesteuerung der Netto-Betrag
(die Vorsteuer bekommst Du vom Finanzamt zurück), als
§19-Kleinunternehmer der **Brutto**-Betrag, weil die gezahlte
Umsatzsteuer für Dich nicht abziehbar ist und zu den
Anschaffungskosten gehört.

Typische Anlagen sind Notebook, Werkstattmaschine, Geschäftswagen,
Büromöbel ab einem gewissen Preis.

Bei Unsicherheit, ob eine konkrete Anschaffung eine Anlage ist
oder sofort absetzbar, frag Deinen Steuerberater.

## Anschaffung anlegen

Klick im Hauptmenü auf "Anschaffungen" und dann auf **"+ Neue
Anschaffung"**. Pflichtfelder sind mit einem Stern markiert. Trage ein:

1. **Bezeichnung** (zum Beispiel "Notebook ThinkPad X1")
2. **Anschaffungsdatum**
3. **Anschaffungskosten** (€): als §19-Kleinunternehmer der
   Brutto-Wert, bei Regelbesteuerung der Netto-Wert
4. **Lieferant (Kontakt, optional)**: ein bestehender Kontakt
5. **AfA-Kategorie (optional)**: Auswahl aus der amtlichen Tabelle des
   Bundesfinanzministeriums; sie setzt Methode und Nutzungsdauer passend
6. **Abschreibungsmethode**, siehe Abschnitt "AfA-Methoden"
7. **Nutzungsdauer** in Jahren, nur bei linearer Abschreibung (bei
   den Sofort-Methoden entfällt sie)
8. **Betrieblicher Anteil** als Zahl von 0 bis 100 %: Bei Misch-Nutzung
   (z. B. Kfz mit privatem Anteil) gibst Du hier den geschäftlichen
   Anteil ein. Unter dem Feld zeigt Klein.Buch die Abschreibungsbasis,
   also den Teil der Anschaffungskosten, der abgeschrieben wird
9. **Notiz (optional)**

Mit **"Methode vorschlagen"** schlägt Klein.Buch Methode und
Nutzungsdauer passend zu Betrag und Kategorie vor; die Begründung steht
als "Vorschlag: …" im Formular. Fehlt eine Pflichtangabe, steht die
Meldung am Feld, und der Cursor springt zum ersten fehlerhaften Feld.

Klick auf **"Anschaffung anlegen"**. Klein.Buch vergibt die Nummer
(AV-Jahr-laufende Nummer, zum Beispiel AV-2026-0001), meldet
"Anschaffung angelegt: …" und öffnet die Anschaffung. Abschreibungen
bucht Klein.Buch erst mit dem AfA-Lauf (siehe "AfA-Lauf"); bis dahin
steht die Anschaffung auf "Noch keine AfA".

### Aus Kosten aktivieren

Hast Du den Kauf schon als Kosten erfasst, öffne die Kosten und klick
in der Karte "Anlagevermögen" auf **"Als Anlage übernehmen"**. Klein.Buch
übernimmt Bezeichnung, Lieferant, das Belegdatum als Anschaffungsdatum
und die Anschaffungskosten: als §19-Kleinunternehmer den Brutto-Betrag, bei
Regelbesteuerung den Netto-Betrag (maßgeblich ist Dein Status bei
Leistungsbezug, also im Jahr des Belegdatums der Kosten, nicht im Jahr
der Zahlung). Das Feld "Anschaffungskosten" ist dann schreibgeschützt;
der Hinweis darunter sagt, ob der Betrag brutto (Kleinunternehmer) oder
netto (Regelbesteuerung) aus den Kosten kommt. Den Methoden-Vorschlag
prüft Klein.Buch am Netto-Betrag der Kosten gegen die GWG-Grenze. Ab dann zählen die Kosten nicht mehr
selbst in der EÜR; die Anschaffung wirkt nur noch über die AfA. Bei
Regelbesteuerung bleibt die gezahlte Vorsteuer eine Ausgabe im Jahr der
Zahlung (Vorsteuer-Zeile der Ausfüllhilfe, im DATEV-Stapel auf dem
Vorsteuerkonto); abgeschrieben wird nur der Netto-Betrag.

Klein.Buch lehnt das Aktivieren ab und nennt den Grund oben auf der
Seite, wenn

- das Geschäftsjahr des Belegs oder der Zahlung schon abgeschlossen ist
  — dort stecken die Kosten bereits in der EÜR,
- die Kosten storniert sind oder schon zu einer Anlage gehören,
- die Anschaffungskosten vom Betrag der Kosten abweichen (als
  §19-Kleinunternehmer der Brutto-Betrag, bei Regelbesteuerung der
  Netto-Betrag). Die EÜR nimmt die ganze Kostenposition heraus; ein
  kleinerer Betrag würde den Rest nie abziehen, ein größerer ohne
  Ausgabe abschreiben,
- das Anschaffungsdatum und das Belegdatum der Kosten in Jahren mit
  unterschiedlichem Steuerstatus liegen (siehe "Steuerstatus und
  Zahldatum").

Ist nur ein Teil einer Sammelrechnung eine Anlage, teile die Rechnung
vorher in getrennte Kostenpositionen auf (je Anlage eine, der Rest als
normale Kosten) und übernimm nur die passende Position. Nebenkosten wie
Versand oder Einrichtung auf derselben Rechnung gehören in dieselbe
Kostenposition. Kommen sie mit einem eigenen Beleg, erfasst Du sie als
eigene Kostenposition und übernimmst sie als eigene Anlage mit derselben
Methode und Nutzungsdauer wie den Gegenstand.

**§13b als Kleinunternehmer.** Bei Reverse-Charge (§13b UStG) schuldest
Du die Umsatzsteuer selbst und kannst sie nicht abziehen; sie gehört zu
den Anschaffungskosten (§9b Abs. 1 EStG). Auf der Rechnung steht sie
nicht, deshalb übernimmst Du die Rechnung zum Rechnungsbetrag (netto =
brutto). Die Steuer zahlst Du mit der Umsatzsteuer-Voranmeldung ans
Finanzamt. Diese Zahlung erfasst Du als eigene Kosten (nur der Anteil
dieser Anlage, Umsatzsteuer 0, ohne §13b-Schalter) und übernimmst sie
als eigene Anlage mit gleichem Anschaffungsdatum, gleicher Methode und
Nutzungsdauer wie den Gegenstand. Beide Kosten zählen dann nicht selbst
in der EÜR, beide Anlagen zusammen schreiben Rechnung und Steuer genau
einmal ab. Trag die Steuer auf die Anlage nicht zusätzlich als normale
Kosten ein. Einen höheren Betrag an der Rechnungsposition lehnt
Klein.Buch mit diesem Hinweis ab.

So führt Dich Klein.Buch dabei:

1. Öffne die Anschaffung aus der Lieferantenrechnung (die
   Hauptanschaffung). Sie zeigt die Karte "Selbst geschuldete Steuer
   (§13b)" mit dem Knopf **"Steuer als Kosten erfassen"**.
2. Der Knopf öffnet "Neue Kosten erfassen", vorbelegt mit dem Lieferanten
   "Finanzamt", einer Beschreibung mit der Nummer der Hauptanschaffung
   und dem USt-Satz 0 %, ohne §13b-Schalter. Trag den Betrag ein (nur
   den Anteil dieser Anschaffung) und klick auf "Kosten festschreiben".
3. Nach der Rückfrage öffnet Klein.Buch direkt "Neue Anschaffung" für
   diese Kosten. In der Auswahl "Selbst geschuldete Steuer (§13b) zu
   Anschaffung" ist die Hauptanschaffung schon gewählt; Anschaffungsdatum,
   Abschreibungsmethode, Nutzungsdauer, AfA-Kategorie und betrieblicher
   Anteil kommen von ihr. Prüfe die Werte und klick auf "Anschaffung
   anlegen".

Die Auswahl erscheint auch, wenn Du solche Kosten später über "Als
Anlage übernehmen" aktivierst; angeboten werden nicht veräußerte
Anschaffungen aus §13b-Kosten als Kleinunternehmer. Ist die
Hauptanschaffung kein geringwertiges Wirtschaftsgut, schlägt Klein.Buch
auch für die Steuer keine Sofortabschreibung vor und weist am Feld
"Abschreibungsmethode" darauf hin, wenn Du sie von Hand wählst. Die
Auswahl belegt nur die Felder vor; Klein.Buch speichert keine
Verknüpfung zwischen den beiden Anschaffungen.

**Steuerstatus und Zahldatum.** Ob Brutto oder Netto, richtet sich nach
Deinem Steuerstatus bei Leistungsbezug (§15 Abs. 1 UStG): Hast Du das
Notebook im Dezember noch als Kleinunternehmer bekommen und zahlst es
erst im Januar nach dem Verzicht auf §19, bleibt es beim Brutto-Betrag;
eine Vorsteuer gibt es dafür nicht. Kosten haben kein eigenes
Leistungsdatum, Klein.Buch nimmt das Belegdatum. Bei einer Anlage ist der
Leistungsbezug allerdings die Lieferung (§9a EStDV), also das
Anschaffungsdatum, das Du eingibst. Liegt es in einem Jahr mit anderem
Steuerstatus als das Belegdatum (zum Beispiel Lieferung im Dezember als
Kleinunternehmer, Rechnung im Januar nach dem Verzicht), kann Klein.Buch
das nicht abbilden und lehnt das Übernehmen oder Bearbeiten mit beiden
Daten ab. Prüfe dann das Anschaffungsdatum; stimmt es, klärst Du die
Erfassung mit Deiner Steuerberatung. Das Zahldatum kannst Du
jederzeit nachtragen oder ändern, es ändert am Steuerstatus
nichts; es bestimmt nur, in welchem Jahr die Ausgabe (bei
Regelbesteuerung die Vorsteuer) in der EÜR steht.

**Nach einem Wechsel des Steuerstatus.** Ein Verzicht auf §19 gilt ab
Beginn des Kalenderjahres, ebenso der Wegfall, weil Dein Vorjahresumsatz
über 25.000 € lag. Hast Du in diesem Jahr schon Kosten als
Anlage übernommen (brutto), passen deren Anschaffungskosten danach nicht
mehr (netto wäre richtig). Klein.Buch blockiert den Wechsel nicht, legt
aber unter "Hinweise" einen Hinweis an, der jede betroffene Anlage mit dem
richtigen Betrag nennt; die EÜR-Ausfüllhilfe zeigt sie ebenfalls. Öffne
die Anschaffung, wähle "Bearbeiten" und korrigiere die
Anschaffungskosten. Ist schon AfA gebucht, setzt Du die Abschreibung des
offenen Geschäftsjahres vorher zurück (siehe "Korrektur"). Ist für die Anlage AfA in einem abgeschlossenen Jahr
festgeschrieben oder die Anlage schon abgegangen, klärst Du die
Korrektur mit Deiner Steuerberatung, ebenso wenn das Anschaffungsdatum
in einem Jahr mit anderem Steuerstatus als das Belegdatum liegt. Eine
Vorsteuerberichtigung nach §15a UStG für Anlagen aus früheren Jahren
(der Wechsel gilt dort als Änderung der Verhältnisse) berechnet
Klein.Buch nicht; klär sie mit Deiner Steuerberatung.

**Wegfall von §19 beim Überschreiten der 100.000-€-Grenze.** Überschreitet
Dein Umsatz im laufenden Jahr 100.000 €, endet §19 mitten im Jahr (§19
Abs. 1 UStG); Kosten, die Du vor dem Überschreiten bezogen hast, bleiben
beim Brutto-Betrag ohne Vorsteuer. Klein.Buch kennt den Steuerstatus nur
je Kalenderjahr: Legst Du den Schalter um, gilt die Regelbesteuerung für
das ganze Jahr, und der Hinweis nennt Deine bisher brutto übernommenen
Anlagen als abweichend. **Korrigiere sie in diesem Fall nicht** — der
Brutto-Betrag ist für Anlagen, die Du vor dem Überschreiten bekommen hast,
richtig. Klär die Erfassung des Jahres mit Deiner Steuerberatung; mehr dazu
im Kapitel "Kleinunternehmer-Regelung".

Anlagen mit abweichendem Betrag (etwa aus einer älteren Version) ändert
Klein.Buch nicht von selbst; die EÜR-Ausfüllhilfe nennt die betroffenen
Kosten und Anlagen mit demselben Weg.

Aktivierte Kosten lassen sich nicht mehr stornieren. Gibst Du den
Gegenstand zurück, erfasst Du das als Abgang der Anlage (Art
"Verkauf", Erlös = Erstattung).

## Liste und Status

Die Liste "Anschaffungen" zeigt je Anschaffung Nummer, Bezeichnung,
Anschaffungsdatum, Anschaffungskosten, Abschreibungsmethode (mit dem
betrieblichen Anteil, wenn er unter 100 % liegt), Restbuchwert und
Status. Du filterst nach Nummer oder Bezeichnung, nach Anschaffungsjahr
und nach "Aktive", "Veräußerte" oder "Alle"; über der Tabelle steht der
Restbuchwert der angezeigten aktiven Anschaffungen. Der Status bedeutet:

- **Noch keine AfA:** angelegt, noch keine Abschreibung gebucht. Du
  kannst die Anschaffung noch bearbeiten.
- **In Abschreibung:** mindestens eine Abschreibung ist gebucht.
- **Festgeschrieben:** das Anschaffungsjahr ist abgeschlossen (Kapitel
  "Geschäftsjahr-Abschluss").
- **Abgeschrieben:** der Restbuchwert ist 0.
- **Veräußert:** der Abgang ist gebucht.

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

Bei eingeschaltetem Automatik-Schalter (Standard, im Bereich
"Geschäftsjahr" unter "Abschreibungen (AfA) zur Jahreswende automatisch
buchen") erzeugt Klein.Buch die AfA-Buchungen zum 1. Januar
jedes Geschäftsjahres automatisch. Du siehst die Buchung in der
Anschaffung unter "Abschreibungs-Verlauf" und in der EÜR als Ausgabe.

Wenn der automatische Lauf abgeschaltet ist, klickst Du in der Liste
"Anschaffungen" oben auf **"Abschreibung buchen"** (auf schmalen
Bildschirmen im Menü "Mehr"). Über der Tabelle öffnet sich der Bereich
"Abschreibung buchen": Wähle das Geschäftsjahr; darunter siehst Du, was
für dieses Jahr schon gebucht ist. Klick auf "AfA … buchen" (mit dem
gewählten Jahr) und bestätige die Rückfrage mit "Jetzt buchen".
Klein.Buch bucht die Jahres-Abschreibung aller Anschaffungen und holt
noch nicht gebuchte Vorjahre ab dem Anschaffungsjahr nach. Tu das
spätestens vor dem Geschäftsjahr-Abschluss. Fehlt beim EÜR-Export noch
AfA, weist die Export-Seite darauf hin und bietet "AfA für … jetzt
buchen" an.

## Anschaffung verkaufen oder verschrotten

Wenn Du eine Anschaffung vor Ablauf der Nutzungsdauer ausscheidest
(Verkauf, Verschrottung, Diebstahl), öffne die Anschaffung und wähle im
Menü "Mehr" **"Veräußern / Entsorgen"**. Trage ein:

1. **Datum des Abgangs**
2. **Art des Abgangs:** "Verkauf", "Verschrottung / Entsorgung" oder
   "Privatentnahme / Geschenk"
3. bei einem Verkauf den **Verkaufserlös** (0, wenn Du nichts erlöst
   hast), bei einer Privatentnahme oder einem Geschenk den
   **Entnahmewert** (Pflichtfeld, siehe unten)

Die Karte "Vorschau" rechnet zum gewählten Datum vom Server vor, was der
Abgang bucht: den Restbuchwert heute, schon gebuchte AfA, die der Abgang
ersetzt (mit "+", sie kommt zum Restbuchwert hinzu), nachgeholte AfA
(mit "−"), die AfA des Abgangsjahres bis zum Abgang und
den Restbuchwert zum Abgang. Beim Verkauf steht daneben der
Veräußerungsgewinn oder -verlust. Ist ein Abgang zu diesem Datum nicht
möglich, steht der Grund sofort am Datumsfeld.
Klick auf **"Abgang bestätigen"**. Eine Rückfrage ("Abgang von … buchen?")
nennt Datum, Art, Erlös bzw. Entnahmewert, den Restbuchwert zum Abgang
und den Gewinn oder Verlust; der Abgang lässt sich nicht rückgängig
machen. Erst **"Abgang buchen"** darin bucht ihn. Klein.Buch beendet die
laufende AfA, verbucht den Rest-Buchwert als Ausgabe und legt den
Verkaufs-Erlös als Einnahme an. Die Anschaffung steht danach auf
"Veräußert" und zeigt die Karte "Veräußerung".

**Privatentnahme / Geschenk.** Nimmst Du die Anschaffung in Dein
Privatvermögen (zum Beispiel das alte Notebook für die Familie),
ist das eine Entnahme: Sie zählt mit dem Zeitwert (Teilwert, § 6
Abs. 1 Nr. 4 EStG) als Betriebseinnahme, in der Anlage EÜR in der
Zeile "Veräußerung oder Entnahme von Anlagevermögen". Trag diesen
Wert als **Entnahmewert** ein; Klein.Buch bucht ihn wie einen Erlös
und den Rest-Buchwert als Ausgabe. Im DATEV-Stapel kommt der Betrag
vom Privatkonto, nicht von der Bank. Schenkst Du die Anschaffung aus
betrieblichem Anlass einem Geschäftspartner, trägst Du 0 ein; für
Geschenke über 50 € gelten die Grenzen nach § 4 Abs. 5 Nr. 1 EStG.
Den Zeitwert und die Konten lässt Du am besten von Deiner
Steuerberatung bestätigen.

Fehlt für ein Vorjahr noch die AfA-Buchung, holt Klein.Buch sie vor dem
Abgang nach; danach bucht es für das Abgangsjahr die anteilige AfA bis
zum Monat des Abgangs. Hast Du für das Abgangsjahr schon die volle
Jahres-AfA oder für spätere Jahre bereits AfA gebucht, ersetzt Klein.Buch
diese Buchungen dabei durch die anteilige AfA; das Protokoll hält die
alten Werte fest. Liegt das Datum vor der Anschaffung oder ist ein
späteres Geschäftsjahr schon abgeschlossen, in dem die Anlage noch im
Bestand steht, lehnt Klein.Buch den Abgang ab (der Grund steht schon vor
dem Bestätigen am Datumsfeld); eine solche Korrektur
klärst Du mit Deiner Steuerberatung. Eine veräußerte Anschaffung
ist abgeschlossen: Ihre Daten, ihre AfA-Buchungen und ihr Rest-Buchwert
lassen sich nicht mehr ändern oder zurücksetzen, auch solange das
Geschäftsjahr noch offen ist.
Prüf die Angaben und die Vorschau deshalb vor dem Bestätigen.

Bei einem Verkauf an einen anderen Unternehmer brauchst Du
zusätzlich eine Verkaufsrechnung. Die legst Du normal im Bereich
"Rechnungen" an. Beachte dabei den Hinweis der Steuer-Übersicht (EÜR):
Erfasst Du den Verkauf zusätzlich als bezahlte Rechnung, zählt der Erlös
in der EÜR doppelt. Wie Du die Rechnung in diesem Fall erfasst, klärst
Du am besten mit Deiner Steuerberatung.

## Korrektur

Vor dem ersten AfA-Lauf kannst Du die Anschaffungs-Daten frei
korrigieren: Öffne die Anschaffung, klick auf **"Bearbeiten"**, ändere
die Angaben und speichere mit "Änderungen speichern". Sobald die erste
AfA-Buchung steht, gibt es "Bearbeiten" nicht mehr. Wenn Du dennoch
korrigieren musst, wählst Du in der Anschaffung im Menü "Mehr"
**"Abschreibung zurücksetzen"** und bestätigst die Rückfrage mit
"Zurücksetzen"; danach kannst Du die Anschaffung wieder bearbeiten.
Das funktioniert nur, solange das Geschäftsjahr nicht abgeschlossen
ist und die Anschaffung nicht veräußert wurde. Nach dem
Geschäftsjahr-Abschluss sind die AfA-Werte für das
abgeschlossene Jahr endgültig fest.

## AfA in der EÜR

Die jährliche AfA-Buchung erscheint in der EÜR als Ausgabe und
schmälert den Gewinn. Sie ist eine reine Buchungs-Ausgabe und nicht
mit einem Geldfluss verbunden, weil Du die Anschaffung ja schon
im Kaufjahr bezahlt hast. Diese Trennung ist eine Ausnahme von der
reinen Cash-Basis und steht ausdrücklich in §4 Abs. 3 Satz 3 EStG.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
