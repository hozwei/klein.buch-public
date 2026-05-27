---
slug: factory-reset
title: Factory Reset
category: bedienen
order: 230
keywords: [factory reset, zurücksetzen, löschen, total-löschung, neuanfang, aufbewahrung]
---

# Factory Reset

Der Factory Reset setzt Klein.Buch komplett zurück. Datenbank,
Beleg-Archiv, lokale Backups und Einstellungen verschwinden. Du
landest wieder im Onboarding wie beim allerersten Start.

Das ist die einzige Funktion in Klein.Buch, die Daten wirklich
löscht. Sie ist als Notfall- und Geräte-Übergabe-Werkzeug gedacht,
nicht als regulärer Aufräum-Mechanismus.

## Wann ein Factory Reset Sinn hat

1. Du gibst den Rechner ab und willst sicherstellen, dass keine
   Reste übrig sind.
2. Du willst Klein.Buch komplett neu beginnen, hast aber noch
   Deine Passphrase.

**Hinweis bei Passphrase-Verlust:** Der Factory Reset verlangt zur
Bestätigung die aktuelle Passphrase und ist damit nur als
"sauberer Neuanfang" gedacht, nicht als Notausgang. Wenn Du die
Passphrase nicht mehr hast, gehst Du außerhalb der App vor: Den
Klein.Buch-Daten-Ordner im Datei-Explorer komplett löschen
(`%APPDATA%\de.wildbach.kleinbuch\`), die App neu starten und mit
neuer Passphrase neu einrichten. Deine alten Belege sind damit
endgültig verloren — die GoBD-Aufbewahrungspflicht musst Du dann
außerhalb von Klein.Buch erfüllen, falls Du Belege exportiert hast.

## Was vorher unbedingt passieren muss

Wenn Du festgeschriebene Belege in Deiner Datenbank hast, bist Du
nach §147 AO verpflichtet, sie 10 Jahre aufzubewahren. Ein
Factory Reset verletzt diese Pflicht, wenn Du die Daten nicht
vorher sicherst.

Klein.Buch zwingt Dich vor dem Reset zu einer von zwei Aktionen:

1. Steuerberater-ZIP exportieren (das ist die Empfehlung) und an
   einem sicheren Ort ablegen.
2. Oder eine getippte Aufbewahrungs-Quittung bestätigen, in der
   Du schriftlich erklärst, dass Du die Aufbewahrungspflicht außerhalb
   von Klein.Buch erfüllst.

Die erste Variante ist die prüfungssichere. Die zweite ist nur
dann sinnvoll, wenn Du sicher weißt, dass die Daten an anderer
Stelle bereits in geeigneter Form vorhanden sind.

## Ablauf

Klick in den Einstellungen auf "Klein.Buch zurücksetzen".
Klein.Buch zeigt Dir eine ausführliche Warnung mit GoBD-Hinweis.

Wenn Du festgeschriebene Belege hast: Klick auf "Steuerberater-ZIP
exportieren" oder bestätige die Aufbewahrungs-Quittung.

Tipp "LÖSCHEN" in das Bestätigungs-Feld. Das verhindert
versehentliche Auslösung. Klein.Buch fragt nach Deiner Passphrase
und prüft sie. Wer die Passphrase nicht hat, kann den Reset nicht
auslösen.

Bestätige den letzten Dialog. Klein.Buch markiert den Reset und
startet die App neu. Beim Neustart leert die App den gesamten
Daten-Bereich (Datenbank, Beleg-Archiv, lokale Sicherungen,
Logo und Branding-Material, Exporte, halbfertige
Wiederherstellungs-Daten)
und legt die Kern-Verzeichnisse leer wieder an. Anschließend
landest Du im Onboarding.

## Was bleibt

Der Reset wischt nur den lokalen Bereich auf diesem Rechner. Was
bestehen bleibt:

1. Off-Site-Backups in der Cloud oder auf einem SFTP-Server.
   Lösche sie gegebenenfalls von Hand, wenn Du sicherstellen
   willst, dass nichts mehr da ist.
2. Bereits exportierte Dateien (Steuerberater-ZIP, PDF-Kopien,
   E-Mail-Anhänge beim Empfänger).
3. Die Klein.Buch-Installation selbst. Den Reset macht nur den
   Datenstand neu, nicht die Anwendung.

## Schlüsselbund-Reste

Klein.Buch räumt beim Reset auch die OS-Schlüsselbund-Einträge auf,
die zur App gehören (SMTP-Passwörter, OAuth-Token, SFTP-Backup-
Passwort). So bleiben keine Geheimnisse zurück, wenn Du den
Rechner abgibst.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
