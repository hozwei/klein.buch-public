---
slug: euer-export
title: EÜR-Export
category: bedienen
order: 190
keywords: [eür-export, elster, datev, steuerberater, zip, ausfüllhilfe, csv, pdf]
---

# EÜR-Export

Klein.Buch erzeugt aus Deiner EÜR mehrere Exporte, je nachdem, wer
der Empfänger ist. Du wählst den passenden zum Zweck.

Den Export-Bereich öffnest Du über das Hauptmenü "Steuer (EÜR)"
und dort über den Link **"Für die Steuererklärung exportieren →"**.
Jeder Export kommt als Download: Dein Browser speichert die Datei in
seinem Download-Ordner. Unter dem Knopf zeigt Klein.Buch danach den
Dateinamen an. Auf dem Server liegt die Datei nur kurz: Sie wird mit dem
Herunterladen gelöscht, spätestens nach zehn Minuten. Bricht der Download
ab, erzeugst Du den Export einfach noch einmal.

## ELSTER-Ausfüllhilfe

Wenn Du die Steuererklärung selbst über das ELSTER-Portal abgibst,
nutzt Du die Ausfüllhilfe. Im Export-Bereich findest Du das Feld
**"ELSTER-Ausfüllhilfe (CSV)"**. Klick dort auf **"CSV
herunterladen"**. Wie bei allen Exporten fragt Klein.Buch vorher noch
einmal Daten-Passwort und App-Code ab (gilt fünf Minuten).

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
Überhang steht dann als Betriebsausgabe in der Ausfüllhilfe) oder bei
Regelbesteuerung, was Du noch ergänzen musst.

Zusätzlich kannst Du die vollständige EÜR als PDF herunterladen
("PDF herunterladen" bei "Vollständige EÜR (PDF)"), als Beleg für
Deine eigene Ablage.

Eine direkte ELSTER-Übermittlung (ERiC) ist in Klein.Buch v1.0
nicht enthalten — sie würde eine separate Zertifikats-Anmeldung
erfordern.

## DATEV-Buchungsstapel

Wenn Du einen Steuerberater hast, nutzt Du den DATEV-Export. Im
Export-Bereich gibt es das Feld **"DATEV-Buchungsstapel
(Steuerberater)"** plus eine Auswahl für den Kontenrahmen (SKR03
ist Default, SKR04 verfügbar). Klick auf **"DATEV herunterladen"**.

Klein.Buch erzeugt eine CSV-Datei im DATEV-Format mit allen
Buchungen des Geschäftsjahres. Die Konto-Zuordnung ist ein
Vorschlag, den Dein Steuerberater vor dem Import in sein System
prüfen sollte. Als Geldkonto nimmt Klein.Buch das Zahlungskonto, das Du
bei der Zahlung angegeben hast: Bank, Kasse oder für PayPal, Stripe und
Sonstiges das Geldtransit-Konto. Ohne Angabe bucht es auf die Bank.
Berater- und Mandantennummer bleiben leer; Dein Steuerberater importiert
den Stapel in den geöffneten Mandanten. Sende die CSV per E-Mail oder
Daten-Übergabe-Dienst.

## Steuerberater-Paket (ZIP)

Der dritte Export ist das komplette Steuerberater-Paket. Du findest
es im selben Export-Bereich unter **"Steuerberater-Paket (ZIP, alles
in einem)"**. Klick auf **"Paket herunterladen"**. Weil das Paket
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

Ist das Geschäftsjahr schon abgeschlossen, vergleicht Klein.Buch beim
Export die EÜR mit dem beim Abschluss festgeschriebenen Stand. Weicht
sie ab, vermerkt Klein.Buch das im Audit-Log und unter "Hinweise" und in
der exportierten Datei selbst: Das EÜR-PDF zeigt oben einen roten Kasten
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

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.10*
