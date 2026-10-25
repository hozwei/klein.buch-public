---
slug: kleinunternehmer-regelung
title: Kleinunternehmer-Regelung
category: recht-und-steuern
order: 300
keywords: [§19, kleinunternehmer, umsatzsteuer, grenze, regelung, paragraph 19]
---

# Kleinunternehmer-Regelung (§19 UStG)

Wer ein kleines Gewerbe oder eine freiberufliche Tätigkeit
betreibt und mit dem Umsatz unter den gesetzlichen Grenzen bleibt,
ist Kleinunternehmer. §19 UStG ist der Paragraph im
Umsatzsteuergesetz, der diese Regelung erklärt.

## Was es bedeutet

Du weist auf Deinen Rechnungen keine Umsatzsteuer aus. Stattdessen
druckt Klein.Buch automatisch den Hinweis "Gemäß §19 UStG wird
keine Umsatzsteuer ausgewiesen" auf jede Rechnung. Das ist
Pflichtangabe und kein optionales Feature.

Im Gegenzug darfst Du keine Vorsteuer ziehen. Wenn Du als
Kleinunternehmer eine Eingangsrechnung mit ausgewiesener
Umsatzsteuer bekommst, ist diese Umsatzsteuer für Dich ein
normaler Bestandteil der Ausgabe. Du buchst den Brutto-Betrag.

Davon getrennt ist §13b UStG zu sehen: Schuldest
Du als Leistungsempfänger die Umsatzsteuer (zum Beispiel
bei Werbung oder Software von Anbietern im EU-Ausland), musst Du sie
auch als Kleinunternehmer anmelden und zahlen, ohne sie als
Vorsteuer abziehen zu dürfen. Klein.Buch berechnet diese Steuer
nicht; wie Du vorgehst, steht im Kapitel "Kosten erfassen" unter
"§13b: Du schuldest die Steuer".

## Die Umsatz-Grenzen (Stand 2026)

Du bist Kleinunternehmer, wenn Du beide Bedingungen erfüllst:

1. Im Vorjahr hattest Du höchstens **25.000 € Gesamtumsatz**
   (Netto-Grenze nach JStG 2024).
2. Im laufenden Jahr bleibst Du unter **100.000 € Umsatz**.

Seit dem JStG 2024 (gültig ab 01.01.2025) sind das harte Grenzen,
keine bloßen Prognosen. **Klein.Buch überwacht die Grenzen nicht**
und warnt Dich nicht, wenn Du Dich ihnen näherst. Behalte Deinen
Umsatz selbst im Blick, zum Beispiel über die Einnahmen in der EÜR.

Sobald Dein laufender Jahresumsatz die 100.000 €-Grenze
überschreitet, fällt der §19-Status **ab diesem Umsatz** sofort
weg — der Umsatz, mit dem die Schwelle gerissen wird, gehört
bereits zur Regelbesteuerung. Alle vorherigen Umsätze des Jahres
bleiben §19. Im Folgejahr bist Du ebenfalls regelbesteuert, weil Dein
Vorjahresumsatz dann über 25.000 € liegt. Liegt Dein Umsatz im
laufenden Jahr nur über 25.000 €, aber unter 100.000 €, bleibst Du
in diesem Jahr Kleinunternehmer; die Regelbesteuerung beginnt dann
erst im Folgejahr. Im Jahr der Gründung gilt im laufenden Jahr schon
die 25.000 €-Grenze; für ihr Überschreiten gilt dasselbe wie unten für
die 100.000 €-Grenze beschrieben.

### Wenn Du die 100.000 €-Grenze mitten im Jahr überschreitest

Klein.Buch kennt den Steuerstatus nur je Kalenderjahr. Einen Wechsel
mitten im Jahr bildet es nicht vollständig ab; der einzige Weg ist der
Schalter "Ich bin Kleinunternehmer" in den Einstellungen. Der Dialog
dort ist für den freiwilligen Verzicht geschrieben; seine Warnung zur
5-Jahres-Bindung betrifft den Verzicht, nicht den gesetzlichen Wegfall
durch die Umsatzgrenze. So gehst Du vor:

- **Einnahmen:** Leg den Schalter um, **bevor** Du die Rechnung
  ausstellst, mit der Du die Grenze überschreitest. Rechnungen behalten
  den Status, mit dem sie ausgestellt wurden: Alle danach ausgestellten
  Rechnungen weisen Umsatzsteuer aus, die früheren bleiben §19. Hast Du
  die Rechnung, mit der Du die Grenze überschritten hast, noch mit dem
  §19-Hinweis ausgestellt, stornierst Du sie und stellst sie mit
  Umsatzsteuer neu aus. Ab dann musst Du Umsatzsteuer-Voranmeldungen
  abgeben; die erstellt Klein.Buch nicht.
- **Kosten:** Kosten, die Du vor dem Überschreiten bezogen hast,
  bleiben ohne Vorsteuerabzug. Klein.Buch wendet die Regelbesteuerung
  aber auf alle Kosten des Jahres an: Die EÜR-Ausfüllhilfe zeigt auch
  für diese Kosten Vorsteuer in Kz 185, und der DATEV-Stapel bucht sie
  mit Vorsteuerabzug. Das musst Du mit Deiner Steuerberatung korrigieren; die
  Ausfüllhilfe weist darauf hin.
- **Anlagen:** Der Inbox-Hinweis nach dem Umlegen nennt Deine bisher
  brutto übernommenen Anlagen als abweichend. **Korrigiere sie in diesem
  Fall nicht** — für Anlagen, die Du vor dem Überschreiten bekommen hast,
  ist der Brutto-Betrag richtig (Kapitel "Anlagen und AfA").

## Der Pflicht-Hinweis

Auf jeder Rechnung steht "Gemäß §19 UStG wird keine Umsatzsteuer
ausgewiesen". Im XRechnung-XML steht derselbe Text im Feld BT-22
(eine technische Anweisung für die Empfänger-Software). Klein.Buch
prüft vor dem Festschreiben, dass das gewählte PDF-Template den
Hinweis tatsächlich druckt. Templates ohne den Pflicht-Hinweis sind
für §19-Rechnungen gesperrt.

## §14c-Schutz

Wenn Du als Kleinunternehmer aus Versehen Umsatzsteuer auf eine
Rechnung schreibst, schuldest Du diese Umsatzsteuer trotzdem dem
Finanzamt. Das nennt sich "§14c-Wirkung" und ist eine bittere
Falle.

Klein.Buch verhindert das aktiv: Solange der Schalter "Ich bin
Kleinunternehmer" angeschaltet ist, sind alle Umsatzsteuer-Felder
im Rechnungs-Formular gesperrt und auf 0. Du kannst gar nicht aus
Versehen einen falschen Wert eintippen.

## Verzicht auf §19

Wenn Du regelbesteuert sein willst (zum Beispiel, weil Du große
Investitionen tätigst und die Vorsteuer ziehen möchtest), legst
Du in den Einstellungen den Schalter um. Klein.Buch warnt Dich
ausdrücklich: Mit dem Verzicht bindest Du Dich für mindestens
5 Jahre an die Regelbesteuerung. Du kannst in dieser Zeit nicht
zurück.

Lies vor dem Wechsel das Kapitel zu §14-Pflichtangaben und sprich
am besten mit einem Steuerberater. Klein.Buch unterstützt die
Regelbesteuerung in Version 1.0 nur eingeschränkt; die volle
Workflow-Tiefe kommt in einer späteren Version.

Bei Regelbesteuerung gilt in den Formularen: Neue Positionen bekommen
die Steuerart S mit 19 % (§12 Abs. 1 UStG); einen anderen Satz, etwa
7 %, setzt Du an der Position. Die Steuerart E (steuerbefreit) gibt es
nur als Kleinunternehmer — Steuerbefreiungen nach §4 UStG kennt
Klein.Buch nicht, und eine Rechnung mit E lehnt es ohne §19 ab. Die
Umsatzsteuer rechnet Klein.Buch je Steuersatz aus der Summe der
Nettobeträge, wie es die E-Rechnung (EN 16931) verlangt. Ein Storno
übernimmt immer die Steuerbehandlung der stornierten Rechnung, auch
wenn Du inzwischen gewechselt hast.

In der EÜR-Ausfüllhilfe stehen Einnahmen aus Rechnungen mit
Regelbesteuerung netto in Zeile 15, die eingenommene Umsatzsteuer in
Zeile 17; Ausgaben stehen netto, die Vorsteuer in einer eigenen Zeile.
Maßgeblich ist der Status, mit dem die Rechnung ausgestellt wurde, nicht
der des Zahlungsjahres. Für Deine Kosten gilt dasselbe Prinzip: Ob Du die
Vorsteuer abziehen darfst, hängt von Deinem Status bei Leistungsbezug ab
(§15 Abs. 1 UStG), nicht vom Jahr der Zahlung. Vorsteuer aus Leistungen,
die Du vor dem Verzicht noch als Kleinunternehmer bezogen hast, ist nicht
abziehbar, auch wenn Du die Rechnung erst danach bezahlst. Klein.Buch nimmt
als Zeitpunkt des Leistungsbezugs das Belegdatum der Kosten; das Jahr der
Ausgabe bleibt das Jahr der Zahlung.

Der Verzicht gilt ab Beginn des Kalenderjahres, in dem Du den Schalter
umlegst. Hast Du in diesem Jahr schon Kosten als Anlage übernommen
(brutto), passen deren Anschaffungskosten nicht mehr. Klein.Buch hindert
Dich nicht am Wechsel, nennt Dir die betroffenen Anlagen aber in der Inbox
und in der EÜR-Ausfüllhilfe; wie Du sie korrigierst, steht im Kapitel
"Anlagen und AfA" unter "Nach einem Wechsel des Steuerstatus". Eine
Vorsteuerberichtigung nach §15a UStG für Anlagen aus früheren Jahren
berechnet Klein.Buch nicht. Hast Du §19 nicht freiwillig aufgegeben, sondern
durch Überschreiten der 100.000 €-Grenze verloren, korrigierst Du die
Anlagen nicht (siehe "Wenn Du die 100.000 €-Grenze mitten im Jahr
überschreitest").

Nicht abgebildet sind die Umsatzsteuer-
Voranmeldung, die an das Finanzamt gezahlte und die vom Finanzamt
erstattete Umsatzsteuer, die Umsatzsteuer auf Anlagenverkäufe und
Entnahmen sowie die Vorsteuer aus Anlagen, die Du nicht aus einer erfassten
Kostenposition übernommen hast (aus Kosten übernommene Anlagen: Vorsteuer in
der Vorsteuer-Zeile des Zahlungsjahres). Diese Werte ergänzt Dein
Steuerberater; die Ausfüllhilfe weist darauf hin.

---

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.11*
