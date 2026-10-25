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
keine bloßen Prognosen. Klein.Buch zeigt Dir den aktuellen Stand
in den Einstellungen.

Sobald Dein laufender Jahresumsatz die 100.000 €-Grenze
überschreitet, fällt der §19-Status **ab diesem Umsatz** sofort
weg — der Umsatz, mit dem die Schwelle gerissen wird, gehört
bereits zur Regelbesteuerung. Alle vorherigen Umsätze des Jahres
bleiben §19. Ab dem Folgejahr bist Du dauerhaft regelbesteuert.
Klein.Buch warnt Dich, sobald die laufende Summe sich dem
Schwellenwert nähert.

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
der des Zahlungsjahres. Nicht abgebildet sind die Umsatzsteuer-
Voranmeldung, die an das Finanzamt gezahlte und die vom Finanzamt
erstattete Umsatzsteuer, die Umsatzsteuer auf Anlagenverkäufe und
Entnahmen sowie die Vorsteuer aus Anlagenkäufen. Diese Werte ergänzt Dein
Steuerberater; die Ausfüllhilfe weist darauf hin.

---

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.11*
