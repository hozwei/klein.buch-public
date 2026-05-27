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

## ELSTER-Ausfüllhilfe

Wenn Du die Steuererklärung selbst über das ELSTER-Portal abgibst,
nutzt Du die Ausfüllhilfe. Im Export-Bereich findest Du das Feld
**"ELSTER-Ausfüllhilfe (CSV)"**. Trag einen Speicherpfad ein und
klick auf **"CSV speichern"**.

Klein.Buch erzeugt eine CSV-Datei mit Deinen EÜR-Werten,
aufgeschlüsselt nach den Zeilen der ELSTER-Anlage EÜR. Du öffnest
dann ELSTER im Browser und tippst die Werte aus der CSV in die
entsprechenden Formularfelder ab.

Zusätzlich kannst Du die komplette Anlage EÜR als PDF speichern
("PDF speichern" daneben) — als Beleg für Deine eigene Ablage.

Eine direkte ELSTER-Übermittlung (ERiC) ist in Klein.Buch v1.0
nicht enthalten — sie würde eine separate Zertifikats-Anmeldung
erfordern.

## DATEV-Buchungsstapel

Wenn Du einen Steuerberater hast, nutzt Du den DATEV-Export. Im
Export-Bereich gibt es das Feld **"DATEV-Buchungsstapel
(Steuerberater)"** plus eine Auswahl für den Kontenrahmen (SKR03
ist Default, SKR04 verfügbar). Klick auf **"DATEV speichern"**.

Klein.Buch erzeugt eine CSV-Datei im DATEV-Format mit allen
Buchungen des Geschäftsjahres. Die Konto-Zuordnung ist ein
Vorschlag, den Dein Steuerberater vor dem Import in sein System
prüfen sollte. Sende die CSV per E-Mail oder Daten-Übergabe-Dienst.

## Steuerberater-Paket (ZIP)

Der dritte Export ist das komplette Steuerberater-Paket. Klick im
Hauptmenü auf "Einstellungen", dann auf "Daten exportieren". Wähle
ein Zielverzeichnis und löse den Export aus. Klein.Buch packt:

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
9. ein `manifest.json` mit einem SHA-256-Prüfwert pro Datei

Der Steuerberater kann das Manifest gegen die ausgelieferten
Dateien prüfen, um sicherzustellen, dass nichts unterwegs verändert
wurde. Klein.Buch importiert das Paket nicht zurück — es ist ein
Einbahn-Export.

## Empfehlung

Wenn Du selbst über ELSTER abgibst: ELSTER-Ausfüllhilfe (CSV) plus
Anlage-EÜR-PDF zur eigenen Ablage.

Wenn Du einen Steuerberater hast: das Steuerberater-Paket aus den
Einstellungen. Es enthält den DATEV-Stapel sowieso.

## Was nicht im Export enthalten ist

Klein.Buch lässt absichtlich aus: persönliche Notizen aus
Kontakten, OAuth-Token, SMTP-Passwörter, die Passphrase, interne
Audit-Log-Details. Alle datenschutz-relevanten Felder sind
ausgespart.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
