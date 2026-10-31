---
slug: kleinunternehmer-regelung
title: Kleinunternehmer-Regelung
category: recht-und-steuern
order: 300
keywords: [§19, kleinunternehmer, umsatzsteuer, grenze, regelung, paragraph 19, regelbesteuerung, verzicht, steuerstatus, wegfall, rückkehr, gesamtumsatz]
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
keine bloßen Prognosen. Maßgeblich ist der Gesamtumsatz nach
vereinnahmten Entgelten, also nach Zahlungseingang und netto, ohne
Umsätze mit Anlagevermögen (§ 19 Abs. 2 UStG).

Sobald Dein laufender Jahresumsatz die 100.000 €-Grenze
überschreitet, fällt der §19-Status **ab diesem Umsatz** sofort
weg — der Umsatz, mit dem die Schwelle gerissen wird, gehört
bereits zur Regelbesteuerung. Alle vorherigen Umsätze des Jahres
bleiben §19. Im Folgejahr bist Du ebenfalls regelbesteuert, weil Dein
Vorjahresumsatz dann über 25.000 € liegt. Liegt Dein Umsatz im
laufenden Jahr nur über 25.000 €, aber unter 100.000 €, bleibst Du
in diesem Jahr Kleinunternehmer; die Regelbesteuerung beginnt dann
erst im Folgejahr. Im Jahr der Gründung gilt im laufenden Jahr schon
die 25.000 €-Grenze; für ihr Überschreiten gilt dasselbe wie für
die 100.000 €-Grenze.

### Was Klein.Buch dabei zeigt

Unter Einstellungen → "Meine Firmendaten", Abschnitt "Steuerstatus",
steht Dein **Gesamtumsatz** des Vorjahres und des laufenden Jahres bis
heute: die Zahlungseingänge auf Deine Rechnungen, netto, abzüglich
der Erstattungen aus Stornos. Klein.Buch kennt nur die hier erfassten
Zahlungen; Umsätze außerhalb von Klein.Buch rechnest Du selbst hinzu.

Erreicht oder überschreitet der Gesamtumsatz eine Grenze, erscheint
dort und im Bereich "Hinweise" ein Grenzhinweis. Er nennt die Rechnung,
mit der die Grenze überschritten wurde, und schlägt ein Wirkungsdatum
vor: das Leistungsdatum dieser Rechnung, frühestens den 1. Januar des
Jahres. Kennt Klein.Buch keinen Umsatz aus früheren Jahren, nimmt es
an, dass das laufende Jahr Dein Gründungsjahr ist, und prüft auch die
25.000 €-Grenze; trifft das nicht zu, gilt für Dich im laufenden Jahr
nur die 100.000 €-Grenze. Klein.Buch prüft die Grenzen auch täglich im
Hintergrund.

**Klein.Buch wechselt nie von selbst.** Ein automatischer Wechsel würde
ohne Dein Wissen Rechnungen mit Umsatzsteuer erzeugen. Den Wechsel
trägst Du selbst ein, am besten nach Rücksprache mit Deiner
Steuerberatung.

## Der Steuerstatus in Klein.Buch

Klein.Buch führt Deinen Steuerstatus als **Verlauf**: Jeder Eintrag
hat ein Wirkungsdatum, einen Status (Kleinunternehmer nach §19 UStG
oder Regelbesteuerung) und einen Grund. Ohne Eintrag gilt die
Kleinunternehmer-Regelung. Einträge werden nie gelöscht oder
geändert; einen irrtümlichen Eintrag widerrufst Du mit Grund, er
bleibt dann durchgestrichen im Verlauf sichtbar (siehe "Einen Eintrag
widerrufen").

Du findest den Verlauf unter Einstellungen → "Meine Firmendaten",
Abschnitt **"Steuerstatus"**. Dort stehen:

- der Status von heute, nach einem Verzicht das Ende der Bindung und
  das Datum, ab dem Du frühestens zurückkannst,
- ein schon eingetragener künftiger Wechsel ("Eingetragen: ab …"),
- Dein Gesamtumsatz und die Grenzhinweise,
- der Verlauf als Tabelle mit den Spalten "Wirkung ab", "Status",
  "Grund" und "Bindung bis" und der Aktion "Widerrufen …" am jüngsten
  Eintrag,
- der Knopf **"Statuswechsel eintragen …"**.

### Welcher Status für welchen Beleg gilt

Klein.Buch rechnet jeden Beleg mit dem Steuerstatus **seines Datums**,
nicht mit dem von heute:

- **Rechnungen:** das Leistungsdatum, bei einem Leistungszeitraum dessen
  Ende, ohne beides das Rechnungsdatum (§ 13 Abs. 1 Nr. 1 UStG). Beim Festschreiben friert
  Klein.Buch den Status der Rechnung ein; ein späterer Eintrag ändert
  sie nicht. Ein Storno übernimmt immer den Status der stornierten
  Rechnung.
- **Angebote:** das Angebotsdatum. Wandelst Du ein Angebot in eine
  Rechnung um, gilt der Status am Leistungsdatum der Rechnung (siehe
  "Angebot und Rechnung über einen Wechsel").
- **Abo-Rechnungen:** das Ende des Abrechnungszeitraums, für den die
  Rechnung entsteht.
- **Kosten:** das Belegdatum. Ob Du Vorsteuer abziehen darfst, hängt
  von Deinem Status bei Leistungsbezug ab (§ 15 Abs. 1 UStG), nicht vom
  Jahr der Zahlung.
- **Pakete:** Fügst Du ein Paket in einen Beleg ein, bekommt die
  Position die Steuerart, die am Datum dieses Belegs gilt.

Die Formulare zeigen das an: Gilt am Leistungsdatum (bzw.
Angebotsdatum oder Termin) die Kleinunternehmer-Regelung, blenden sie
die USt-Felder aus. Weicht der Status am Datum des Belegs von dem von
heute ab, steht oben ein Hinweis, etwa "Am Leistungsdatum gilt die
Regelbesteuerung." Änderst Du das Datum über einen Wechsel hinweg,
stellt das Formular die Steuerspalten sofort um.

## Einen Statuswechsel eintragen

Geh auf Einstellungen → "Meine Firmendaten", Abschnitt "Steuerstatus",
und klick auf **"Statuswechsel eintragen …"** (oder im Grenzhinweis auf
"Wegfall eintragen …", dann ist alles vorbelegt). Der Dialog
"Statuswechsel eintragen?" fragt:

1. **Grund** (Pflicht). Zur Wahl stehen nur Gründe, die zum jüngsten
   Eintrag passen: Gilt dort die Kleinunternehmer-Regelung, die Wege
   zur Regelbesteuerung; gilt die Regelbesteuerung, die Rückkehr. Jeder
   Grund trägt eine kurze Erklärung.
2. **Wirkungsdatum** (Pflicht, außer beim Stand zu Beginn). Klein.Buch
   schlägt ein Datum vor. Bei Verzicht, Wegfall wegen des Vorjahres und
   Rückkehr nur zum 1. Januar.
3. **Beleg, der die Grenze überschreitet** (nur bei einem Wegfall im
   laufenden Jahr oder im Gründungsjahr, freiwillig) und eine
   **Notiz** (freiwillig, z. B. das Datum Deiner Erklärung ans
   Finanzamt).

Darunter prüft Klein.Buch den Eintrag und zeigt die **Folgen**, bevor
etwas gespeichert ist: festgeschriebene Rechnungen, deren Status nicht
mehr passt, betroffene Entwürfe und Abo-Vorlagen, aus Kosten
übernommene Anlagen und die Jahre, deren EÜR-, ELSTER- und DATEV-Exporte
Du neu erstellen solltest. Passt der Eintrag nicht (etwa ein Datum vor
dem jüngsten Eintrag, eine Rückkehr während der Bindung oder ein
abgeschlossenes Geschäftsjahr), stehen die Gründe im Dialog. Unter
"Bitte prüfen." stehen Warnungen, die das Eintragen nicht sperren: ein
Verzicht nach Ablauf der Erklärungsfrist oder eine Rückkehr, obwohl Dein
Gesamtumsatz im Vorjahr nach den in Klein.Buch erfassten Zahlungen über
25.000 € lag.

Mit dem Knopf unten rechts ("Verzicht eintragen", "Wegfall eintragen",
"Rückkehr eintragen" bzw. "Regelbesteuerung eintragen") trägst Du den
Wechsel ein; "Abbrechen" lässt alles, wie es war. Weil der Wechsel
geschützt ist, fragt Klein.Buch danach im Dialog "Aktion bestätigen:
Steuerstatus ändern" Dein Daten-Passwort und den App-Code ab, wenn
Deine letzte Bestätigung länger als fünf Minuten her ist. Eine Meldung
bestätigt den Eintrag; betroffene Belege nennt Klein.Buch zusätzlich im
Bereich "Hinweise".

### Die Gründe

- **Stand zu Beginn: Regelbesteuerung.** Du warst schon vor Deinen
  ersten Belegen in Klein.Buch regelbesteuert. Der Eintrag gilt seit
  Beginn der Aufzeichnungen und ist nur möglich, solange es keinen
  Eintrag gibt. Trag ihn gleich bei der Einrichtung ein, vor der
  ersten Rechnung.
- **Verzicht auf §19 (§ 19 Abs. 3 UStG).** Du verzichtest freiwillig,
  zum Beispiel weil Du große Investitionen tätigst und die Vorsteuer
  ziehen möchtest. Der Verzicht wirkt ab dem 1. Januar des Jahres, für
  das er gilt, und **bindet Dich fünf Kalenderjahre einschließlich
  dieses Jahres**: Bei einem Verzicht ab 2026 kannst Du frühestens zum
  01.01.2031 zurück. Der Dialog nennt das Ende der Bindung, bevor Du
  bestätigst. Erklären kannst Du den Verzicht bis Ende Februar des
  übernächsten Jahres (für 2026 also bis 29.02.2028); Klein.Buch lässt
  ihn rückwirkend aber nur für ein offenes Geschäftsjahr zu. Ist die
  Frist abgelaufen, warnt der Dialog: Trag den Verzicht dann nur ein,
  wenn Du ihn rechtzeitig gegenüber dem Finanzamt erklärt hast.
- **Wegfall: Vorjahresumsatz über 25.000 €.** Die Regelung gilt ab dem
  1. Januar nicht mehr, ohne Bindung. Trag den Wegfall ein, bevor Du
  die erste Rechnung des Jahres ausstellst.
- **Wegfall: Umsatz im laufenden Jahr über 100.000 €.** Die Regelung
  endet mit dem Umsatz, der die Grenze überschreitet; als
  Wirkungsdatum gilt das Leistungsdatum dieser Rechnung. Ohne Bindung.
  Das Datum darf nicht in der Zukunft liegen: Den Wegfall trägst Du
  ein, sobald der Umsatz vorliegt.
- **Wegfall: Umsatz im Gründungsjahr über 25.000 €.** Wie beim
  laufenden Jahr, nur mit der Grenze von 25.000 €.
- **Rückkehr zu §19.** Zum 1. Januar eines Jahres, wenn Dein
  Gesamtumsatz im Vorjahr höchstens 25.000 € betrug und im laufenden
  Jahr voraussichtlich höchstens 100.000 € beträgt. Nach einem Wegfall
  geht das ohne Sperre, nach einem Verzicht erst nach Ablauf der fünf
  Jahre. Klein.Buch prüft die Bindung; lag Dein Gesamtumsatz im
  Vorjahr nach den erfassten Zahlungen über 25.000 €, warnt der Dialog.
  Die Umsatzvoraussetzungen prüfst Du selbst.

Der Wechsel kann eine Vorsteuerberichtigung nach §15a UStG auslösen,
wenn Du Anlagen weiter nutzt, die Du unter dem anderen Status
angeschafft hast: Der Übergang zwischen Kleinunternehmer-Regelung und
Regelbesteuerung gilt dort als Änderung der Verhältnisse. Klein.Buch
zeigt dafür die Übersicht **"Vorsteuerberichtigung (§ 15a UStG)"**
(Zeitraum, Jahresanteil, Grenzen nach §44 UStDV): unter "Anschaffungen"
über "Vorsteuerberichtigung (§ 15a)" oder über den Link im Abschnitt
"Steuerstatus" unter "Meine Firmendaten". Nach dem Wechsel legt
Klein.Buch einen Hinweis an, der darauf verweist; gebucht wird nichts
(Kapitel "Anschaffungen und AfA").
Kläre die Beträge mit Deiner Steuerberatung. Die Hinweise in Klein.Buch
ersetzen keine Steuerberatung.

### Wenn Du die 100.000 €-Grenze mitten im Jahr überschreitest

So gehst Du vor:

- **Einnahmen:** Trag den Wegfall ein, sobald Du den Grenzhinweis
  siehst, am besten **bevor** Du die Rechnung festschreibst, mit der Du
  die Grenze überschreitest. Ab dem Wirkungsdatum weisen Rechnungen
  Umsatzsteuer aus; Rechnungen mit früherem Leistungsdatum bleiben
  §19. Hast Du die Rechnung, mit der Du die Grenze überschritten hast,
  schon mit dem §19-Hinweis festgeschrieben, ändert Klein.Buch sie
  nicht, nennt sie aber unter "Hinweise". Kläre mit Deiner
  Steuerberatung, ob Du sie berichtigst (§ 31 Abs. 5 UStDV) oder
  stornierst und mit Umsatzsteuer neu ausstellst. Ab dann musst Du
  Umsatzsteuer-Voranmeldungen abgeben; die erstellt Klein.Buch nicht.
- **Kosten:** Kosten mit einem Belegdatum vor dem Wirkungsdatum
  bleiben ohne Vorsteuerabzug, Kosten ab dem Wirkungsdatum mit. Die
  EÜR-Ausfüllhilfe und der DATEV-Stapel teilen das Jahr entsprechend
  auf.
- **Anlagen:** Für Anlagen, die Du vor dem Wirkungsdatum bekommen
  hast, bleibt der Brutto-Betrag richtig (Kapitel "Anschaffungen und
  AfA").

### Wenn Dein Vorjahresumsatz über 25.000 € lag

Dann giltst Du ab dem 1. Januar des laufenden Jahres nicht mehr als
Kleinunternehmer, ohne 5-Jahres-Bindung. Trag den Wegfall mit dem
Grund "Wegfall: Vorjahresumsatz über 25.000 €" zum 1. Januar ein,
bevor Du die erste Rechnung des Jahres festschreibst. Hast Du in
diesem Jahr schon Rechnungen mit dem §19-Hinweis festgeschrieben,
nennt Klein.Buch sie unter "Hinweise"; es gilt dasselbe wie beim
Überschreiten der 100.000 €-Grenze.

### Angebot und Rechnung über einen Wechsel

Ein Angebot behält den Status seines Datums. Wandelst Du es nach
einem Wechsel in eine Rechnung um, gilt der Status am Leistungsdatum
der Rechnung, und das Formular sagt "Anderer Steuerstatus als im
Angebot.":

- Angebot als Kleinunternehmer, Rechnung mit Regelbesteuerung: Die
  Positionen sind mit 19 % vorbelegt, die Nettopreise bleiben. Prüf die
  Steuersätze und sprich den Bruttobetrag mit Deinem Kunden ab.
- Angebot mit Regelbesteuerung, Rechnung als Kleinunternehmer: Die
  Positionen werden ohne Umsatzsteuer übernommen, die Nettopreise
  bleiben.

### Abo-Rechnungen über einen Wechsel

Jeder Termin einer Abo-Vorlage zählt mit dem Status an seinem Datum.
Passen die Positionen der Vorlage an einem Termin nicht zum Status
(zum Beispiel Steuerart "steuerbefreit" nach einem Wechsel zur
Regelbesteuerung), erzeugt Klein.Buch für diesen Termin keine Rechnung,
sondern einen Hinweis. Pass dann die Positionen der Vorlage an.
Nachgeholte Termine vor dem Wechsel bleiben §19.

## Einen Eintrag widerrufen

Hast Du einen Wechsel irrtümlich eingetragen, klick im Verlauf beim
jüngsten Eintrag auf **"Widerrufen …"**. Der Dialog "Eintrag
widerrufen?" nennt den Status, der danach wieder gilt, und verlangt
einen Grund. Mit "Eintrag widerrufen" bestätigst Du; Klein.Buch fragt
dazu wie beim Eintragen Daten-Passwort und App-Code ab ("Aktion
bestätigen: Steuerstatus-Eintrag widerrufen").

Widerrufen lässt sich nur der jüngste gültige Eintrag, und nur, wenn
sich dadurch kein abgeschlossenes Geschäftsjahr ändert. Willst Du
einen älteren Eintrag korrigieren, widerrufst Du zuerst die jüngeren.
Der widerrufene Eintrag bleibt mit Datum und Grund im Verlauf sichtbar.
Festgeschriebene Rechnungen ändern sich auch hier nicht; betroffene
nennt Klein.Buch unter "Hinweise" in einem eigenen Hinweis zum
Widerruf, etwa Rechnungen, die Du unter dem widerrufenen Status mit
Umsatzsteuer festgeschrieben hast.

## Rückwirkende Einträge und abgeschlossene Jahre

Ein neuer Eintrag muss nach dem jüngsten Eintrag wirken; Einträge
überschneiden sich nie. Rückwirkend eintragen kannst Du nur in einem
**offenen** Geschäftsjahr. Ein Eintrag, der den Status in einem
abgeschlossenen Geschäftsjahr ändern würde, lehnt Klein.Buch ab: Die
Werte dieses Jahres sind beim Jahresabschluss festgehalten. Das gilt
auch für die Vorsteuer von Kosten, die Du als Anlage übernommen und im
abgeschlossenen Jahr bezahlt hast, deren Belegdatum aber im offenen
Jahr liegt: Sie zählt im Jahr der Zahlung, ihr Status richtet sich nach
dem Belegdatum. Eine Korrektur eines abgeschlossenen Jahres klärst Du
mit Deiner Steuerberatung.

Nach einem rückwirkenden Eintrag ändern sich EÜR, ELSTER-Ausfüllhilfe
und DATEV-Stapel der offenen Jahre ab dem Wirkungsdatum (vor allem die
Vorsteuer Deiner Kosten). Der Dialog und die Hinweise nennen die
Jahre; erstelle die Exporte dafür neu.

## Der Pflicht-Hinweis

Auf jeder Rechnung steht "Gemäß §19 UStG wird keine Umsatzsteuer
ausgewiesen". Im XRechnung-XML steht derselbe Text im Feld BT-22
(eine technische Anweisung für die Empfänger-Software). Klein.Buch
prüft vor dem Ausstellen, dass die gewählte Vorlage den
Hinweis tatsächlich druckt. Vorlagen ohne den Pflicht-Hinweis sind
für §19-Rechnungen gesperrt: Unter Einstellungen → "Rechnungs-Layout"
tragen sie den Vermerk "Ohne §19-Klausel" und lassen sich nicht als
Standard wählen, solange heute oder ab einem eingetragenen künftigen
Wechsel die Kleinunternehmer-Regelung gilt.

## §14c-Schutz

Wenn Du als Kleinunternehmer aus Versehen Umsatzsteuer auf eine
Rechnung schreibst, schuldest Du diese Umsatzsteuer trotzdem dem
Finanzamt. Das nennt sich "§14c-Wirkung" und ist eine bittere
Falle.

Klein.Buch verhindert das aktiv: Gilt am Leistungsdatum der Rechnung
die Kleinunternehmer-Regelung, blendet das Rechnungs-Formular die
Umsatzsteuer-Felder ("USt-Satz", "USt-Art") aus und sagt das in einem
Hinweis oben im Formular; alle Positionen laufen ohne Umsatzsteuer. Du
kannst gar nicht aus Versehen einen falschen Wert eintippen. Beim
Festschreiben prüft Klein.Buch den Status am Leistungsdatum noch
einmal.

## Bei Regelbesteuerung

Lies vor dem Wechsel das Kapitel zu §14-Pflichtangaben und sprich
am besten mit einem Steuerberater. Klein.Buch unterstützt die
Regelbesteuerung nur eingeschränkt; die volle
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
Maßgeblich ist der Status, mit dem die Rechnung festgeschrieben wurde,
nicht der des Zahlungsjahres. Für Deine Kosten gilt dasselbe Prinzip:
Ob Du die Vorsteuer abziehen darfst, hängt von Deinem Status am
Belegdatum ab, nicht vom Jahr der Zahlung. Vorsteuer aus Leistungen,
die Du vor dem Wechsel noch als Kleinunternehmer bezogen hast, ist
nicht abziehbar, auch wenn Du die Rechnung erst danach bezahlst; das
Jahr der Ausgabe bleibt das Jahr der Zahlung. Die Steuer-Übersicht
(EÜR) zeigt für jedes Jahr mit Regelbesteuerung den Hinweis
"Regelbesteuerung …" und bei einem Wechsel im Jahr "Wechsel des
Steuerstatus …" mit den Zeiträumen. Getrennt nach Netto und
Umsatzsteuer siehst Du die Beträge im EÜR-Export.

Hast Du schon Kosten als Anlage übernommen (brutto) und trägst danach
einen Wechsel ein, der ihr Belegdatum erfasst, passen deren
Anschaffungskosten nicht mehr. Klein.Buch hindert Dich nicht am
Wechsel, nennt Dir die betroffenen Anlagen aber in der Vorschau des
Dialogs, unter "Hinweise" und in der EÜR-Ausfüllhilfe; wie Du sie
korrigierst, steht im Kapitel "Anschaffungen und AfA" unter "Nach
einem Wechsel des Steuerstatus".

Nicht abgebildet sind die Umsatzsteuer-
Voranmeldung, die an das Finanzamt gezahlte und die vom Finanzamt
erstattete Umsatzsteuer, die Umsatzsteuer auf Anlagenverkäufe und
Entnahmen sowie die Vorsteuer aus Anlagen, die Du nicht aus einer erfassten
Kostenposition übernommen hast (aus Kosten übernommene Anlagen: Vorsteuer in
der Vorsteuer-Zeile des Zahlungsjahres). Diese Werte ergänzt Dein
Steuerberater; die Ausfüllhilfe weist darauf hin.

## Bestand aus früheren Versionen

Bis Klein.Buch 2026.11.0 gab es statt des Verlaufs einen Schalter
"Kleinunternehmer nach §19 UStG". Beim Update übernimmt Klein.Buch
den Stand so, dass für jedes Jahr derselbe Status gilt wie bisher:
Warst Du regelbesteuert, steht im Verlauf ein Eintrag "Wechsel aus dem
bisherigen Schalter übernommen" zum 1. Januar des Jahres, in dem Du
umgeschaltet hattest. Er bindet Dich wie ein Verzicht fünf
Kalenderjahre einschließlich dieses Jahres, weil der Schalter den
Grund nicht kannte. War es in Wahrheit ein Wegfall durch eine
Umsatzgrenze, widerrufe den übernommenen Eintrag (sofern dadurch kein
abgeschlossenes Jahr betroffen ist) und trag den Wechsel mit dem
richtigen Grund neu ein.

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
