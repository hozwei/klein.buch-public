---
slug: euer-export
title: EÜR-Export
category: bedienen
order: 190
keywords: [eür-export, elster, datev, steuerberater, zip, ausfüllhilfe, csv, pdf, beraternummer, mandantennummer, kontenrahmen]
---

# EÜR-Export

Klein.Buch erzeugt aus Deiner EÜR mehrere Exporte, je nachdem, wer
der Empfänger ist. Du wählst den passenden zum Zweck.

Den Export-Bereich öffnest Du über das Hauptmenü "Steuer (EÜR)"
und dort über den Link **"Für die Steuererklärung exportieren →"**.
Die Seite "EÜR-Export" zeigt das Jahr aus der Auswahl "Geschäftsjahr"
oben. Unter der Karte "Exportieren" siehst Du vorab, was in den Dateien
steht: die Zeilen der Anlage EÜR mit Kennzahl, das Anlageverzeichnis
und die Einzelaufstellungen der Einnahmen und Ausgaben, mit Daten im
Format TT.MM.JJJJ.

Jeder Export kommt als Download: Dein Browser speichert die Datei in
seinem Download-Ordner. Unter dem Knopf zeigt Klein.Buch danach
"Heruntergeladen:" mit dem Dateinamen an. Jede Datei wird beim Klick neu
erzeugt und lässt sich nur einmal abrufen. Auf dem Server liegt sie nur
kurz: Sie wird mit dem Herunterladen gelöscht, spätestens nach zehn
Minuten. Bricht der Download ab, erzeugst Du den Export einfach noch
einmal.

Alle Exporte sind geschützt. Beim ersten Export fragt Klein.Buch im
Dialog "Aktion bestätigen: …" (zum Beispiel "Aktion bestätigen:
Steuerberater-Paket herunterladen") nach Deinem Daten-Passwort und dem
Code aus der Authenticator-App (oder einem Einmal-Code). Danach gilt die
Bestätigung fünf Minuten lang auch für weitere geschützte Aktionen.
"Abbrechen" ändert nichts; Klein.Buch meldet dann "Abgebrochen – nichts
geändert."

Fehlt für das gewählte Jahr noch die Abschreibung einer Anschaffung,
warnt die Seite, dass sie im Export fehlen würde, und bietet
**"AfA für … jetzt buchen"** an (Kapitel "Anschaffungen und AfA").

## ELSTER-Ausfüllhilfe

Wenn Du die Steuererklärung selbst über das ELSTER-Portal abgibst,
nutzt Du die Ausfüllhilfe. Im Export-Bereich findest Du das Feld
**"ELSTER-Ausfüllhilfe (CSV)"**. Klick dort auf **"CSV
herunterladen"**. Wie bei allen Exporten fragt Klein.Buch vorher noch
einmal Daten-Passwort und App-Code ab, wenn Deine letzte Bestätigung
länger als fünf Minuten her ist.

Klein.Buch erzeugt eine CSV-Datei mit Deinen EÜR-Werten,
aufgeschlüsselt nach Zeile und Kennzahl (Kz) der amtlichen Anlage EÜR
des jeweiligen Jahres. Die Zeilennummern ändern sich gelegentlich
(2026 zum Beispiel ab „Waren“ um eine bis zwei Zeilen), die Kennzahlen
bleiben gleich — im Zweifel gilt die Kennzahl. Du öffnest dann ELSTER
im Browser und tippst die Werte aus der CSV in die entsprechenden
Formularfelder ab. Kategorien mit eigener Zeile (z. B. Telefon/Internet,
Fortbildung, Bürobedarf als Arbeitsmittel, Werbung) stehen dort,
Hardware, Gebühren und Sonstiges unter „übrige Betriebsausgaben“.
Kommentarzeilen am Anfang der Datei (`# …`) nennen Besonderheiten, etwa
wenn Storno-Erstattungen die Einnahmen des Jahres übersteigen (der
Überhang steht dann als Betriebsausgabe in der Ausfüllhilfe), bei
Regelbesteuerung, was Du noch ergänzen musst, und bei einem Wechsel des
Steuerstatus im Jahr die Zeiträume mit ihrem Status. Die Seite nennt
den Wechsel auch über der Tabelle der Anlage EÜR.

Zusätzlich kannst Du die vollständige EÜR als PDF herunterladen
("PDF herunterladen" bei "Vollständige EÜR (PDF)"), als Beleg für
Deine eigene Ablage.

Eine direkte ELSTER-Übermittlung (ERiC) ist in Klein.Buch
nicht enthalten — sie würde eine separate Zertifikats-Anmeldung
erfordern.

## DATEV-Buchungsstapel

Wenn Du einen Steuerberater hast, nutzt Du den DATEV-Export. Im
Export-Bereich steht er im Abschnitt **"Für die Steuerberatung"**. Dort
legst Du vorher fest, was für den DATEV-Stapel und das
Steuerberater-Paket gilt:

1. **Kontenrahmen:** SKR03 (Standard) oder SKR04
2. **Beraternummer** (optional, 1001 bis 9999999)
3. **Mandantennummer** (optional, 1 bis 99999)

Berater- und Mandantennummer bekommst Du von Deiner Steuerberatung; sie
stehen dann im Kopf des DATEV-Stapels. Liegt eine Nummer außerhalb des
Bereichs, steht die Meldung am Feld, und der Export startet nicht.
Kontenrahmen und Nummern merkt sich Dein Browser für den nächsten
Export; ein anderer Browser oder ein anderes Gerät kennt sie nicht.
Klick dann bei **"DATEV-Buchungsstapel (Steuerberater)"** auf
**"DATEV herunterladen"**.

Klein.Buch erzeugt eine CSV-Datei im DATEV-Format mit allen
Buchungen des Geschäftsjahres. Die Konto-Zuordnung ist ein
Vorschlag, den Dein Steuerberater vor dem Import in sein System
prüfen sollte. Als Geldkonto nimmt Klein.Buch das Zahlungskonto, das Du
bei der Zahlung angegeben hast: Bank, Kasse oder für PayPal, Stripe und
Sonstiges das Geldtransit-Konto. Ohne Angabe bucht es auf die Bank.
Lässt Du Berater- und Mandantennummer leer, importiert Dein
Steuerberater den Stapel in den gerade geöffneten Mandanten. Sende die
CSV per E-Mail oder Daten-Übergabe-Dienst.

## Steuerberater-Paket (ZIP)

Der dritte Export ist das komplette Steuerberater-Paket. Du findest
es im selben Abschnitt "Für die Steuerberatung" unter
**"Steuerberater-Paket (ZIP, alles in einem)"**. Kontenrahmen,
Berater- und Mandantennummer gelten auch für den DATEV-Stapel im Paket.
Klick auf **"Paket herunterladen"**. Weil das Paket
alle Belege enthält, fragt Klein.Buch vorher noch einmal Dein
Daten-Passwort und den App-Code aus Deiner Authenticator-App ab; die
Bestätigung gilt fünf Minuten. Klein.Buch packt:

1. ein Deckblatt-PDF und eine `LIESMICH.txt`
2. die Anlage-EÜR als PDF
3. den DATEV-Buchungsstapel als CSV
4. die ELSTER-Ausfüllhilfe als CSV
5. Einzelaufstellungen (Einnahmen, Ausgaben, Anlagenverzeichnis)
   als CSV
6. Stammdaten als JSON
7. alle ausgestellten Rechnungen als PDF inklusive
   XRechnung-XML
8. alle Kosten-Belege
9. die Liste `nicht-festgeschriebene-nummern.csv`: jede Rechnungs-
   und Storno-Nummer des Jahres ohne ausgestellten Beleg, also
   verworfene Entwürfe (mit Zeitpunkt und Grund) und noch offene
   Entwürfe. Damit kann der Steuerberater jede Lücke in der
   Nummernfolge der Belege nachvollziehen; die `LIESMICH.txt` nennt
   die Anzahl.
10. ein `manifest.json` mit einem SHA-256-Prüfwert pro Datei

Der Steuerberater kann das Manifest gegen die ausgelieferten
Dateien prüfen, um sicherzustellen, dass nichts unterwegs verändert
wurde. Die einzeln exportierte Einzelaufstellung (ZIP) hat bewusst kein
Manifest: Sie ist eine Arbeitsliste ohne Original-Belege; für die
Übergabe ist das Steuerberater-Paket gedacht. Klein.Buch importiert das Paket nicht zurück — es ist ein
Einbahn-Export.

## Protokoll und abgeschlossene Jahre

Jeder Export steht im Audit-Log, mit Geschäftsjahr, Dateiname und
Anzahl der Positionen, aber ohne Inhalte. So ist nachvollziehbar,
wann welche Auswertung das Haus verlassen hat.

Ist das Geschäftsjahr schon abgeschlossen, vergleicht Klein.Buch die
EÜR mit dem beim Abschluss festgeschriebenen Stand. Weicht sie ab, zeigt
die Export-Seite schon beim Öffnen die Warnung "Abweichung vom
Jahresabschluss." mit den betroffenen Werten; das Öffnen selbst schreibt
nichts. Erst ein Export vermerkt die Abweichung im Audit-Log, unter
"Hinweise" und in der exportierten Datei selbst: Das EÜR-PDF zeigt oben einen roten Kasten
mit festgeschriebenen und heutigen Werten, die ELSTER-Ausfüllhilfe eine
Kommentarzeile `# ACHTUNG: …` unter dem Titel, die Einzelaufstellung
eine zusätzliche Datei `HINWEIS-ABWEICHUNG-JAHRESABSCHLUSS.txt`, und das
Steuerberater-Paket nennt beide Werte am Anfang der `LIESMICH.txt`
(Kapitel "Geschäftsjahr-Abschluss"). Nur der DATEV-Buchungsstapel bleibt
unverändert, weil sein Format keine Zusatzzeilen zulässt; die Abweichung
steht dann im Audit-Log und unter "Hinweise".

## Empfehlung

Wenn Du selbst über ELSTER abgibst: ELSTER-Ausfüllhilfe (CSV) plus
Anlage-EÜR-PDF zur eigenen Ablage.

Wenn Du einen Steuerberater hast: das Steuerberater-Paket. Es
enthält den DATEV-Stapel sowieso.

## Was nicht im Export enthalten ist

Klein.Buch lässt absichtlich aus: persönliche Notizen aus
Kontakten, Passwörter von Postfächern und Backup-Zielen, die Passphrase, interne
Audit-Log-Details. Alle datenschutz-relevanten Felder sind
ausgespart.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
