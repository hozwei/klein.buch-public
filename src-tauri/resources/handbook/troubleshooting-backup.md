---
slug: troubleshooting-backup
title: Backup-Fehler
category: troubleshooting
order: 520
keywords: [backup, fehler, ziel, schreibrecht, voll, cloud, fehlgeschlagen]
---

# Backup-Fehler

Wenn Klein.Buch ein Backup nicht schreiben kann, landet ein
Fehler-Eintrag im Backup-Protokoll und ein Eintrag unter "Hinweise".
Hier die typischen Ursachen und die jeweilige Lösung.

## Ziel-Ordner nicht gefunden

Symptom: "Pfad existiert nicht" oder "Zugriff verweigert".

Mögliche Ursachen:

1. Externer USB-Stick ist nicht angesteckt.
2. Netzlaufwerk ist nicht verbunden.
3. Cloud-Ordner ist nicht synchronisiert (zum Beispiel weil der
   Cloud-Client gerade deinstalliert wurde).
4. Du hast den Ziel-Ordner umbenannt oder verschoben.

Lösung: Geh in "Einstellungen" und "Backups". Korrigier in der
Karte "Off-Site-Kopie" den Pfad oder wähl über "Durchsuchen…" den
aktuellen Ordner und klick auf "Speichern". Klein.Buch versucht
mit dem nächsten Backup-Lauf den neuen Pfad.

## Schreibrechte fehlen

Symptom: "Zugriff verweigert" trotz vorhandener Datei-Hierarchie.

Lösung: Im Datei-Explorer das Eigenschaften-Dialog des
Backup-Ordners öffnen, im Reiter "Sicherheit" das eigene
Benutzer-Konto auswählen und "Vollzugriff" gewähren. Auf
Netzlaufwerken oder Unternehmens-Pfaden frag den Administrator.

## Festplatte voll

Symptom: "Kein Speicherplatz" oder "Schreiben fehlgeschlagen
(Code 112)".

Lösung: Auf dem Ziel-Laufwerk Speicher freiräumen. Bei Cloud-
Ordnern: Im Cloud-Anbieter-Konto prüfen, ob das Speicher-Kontingent
ausgeschöpft ist. Klein.Buch-Backups sind in der Regel klein (oft
unter 10 MB), aber andere Dateien können das Laufwerk füllen.

## SFTP-Server nicht erreichbar

Symptom: "Verbindung zum SFTP-Server fehlgeschlagen" oder "Timeout".

Mögliche Ursachen:

1. Server ist offline oder in Wartung.
2. Internet-Verbindung weg.
3. Firewall blockiert den SFTP-Port.

Lösung: Internet-Verbindung prüfen. Server-Adresse und Port noch
einmal kontrollieren. Beim Anbieter nachfragen, ob der Server
gerade läuft. Klein.Buch versucht den nächsten Backup-Zeitpunkt
erneut.

## Host-Key-Wechsel beim SFTP

Symptom: "Host-Key passt nicht mehr".

Das ist ein Sicherheits-Hinweis. Entweder hat der Server-Betreiber
den Schlüssel ausgetauscht (zum Beispiel nach einer
Server-Wartung) oder es findet ein Angriff statt, der Dich auf
einen falschen Server umleitet.

Lösung: Frag beim Server-Anbieter, ob ein Schlüssel-Wechsel
stattgefunden hat. Wenn ja, lass Dir den neuen Fingerabdruck
bestätigen, klick in Klein.Buch auf "Verbindung testen" — der
neue Fingerabdruck erscheint im Test-Ergebnis. Vergleich ihn mit
dem vom Anbieter und pinn ihn mit "Als SFTP-Ziel speichern". Wenn
der Anbieter nichts ausgetauscht hat, brich ab und untersuche die
Sache.

## Cloud-Ordner zeigt nicht alle Dateien

Symptom: Backup-Protokoll meldet Erfolg, im Cloud-Ordner siehst Du
die Datei aber nicht.

Wahrscheinliche Ursache: Cloud-Client hat die Synchronisation
gestoppt oder es gibt einen Konflikt. Lösung: Im Cloud-Client-
Tray-Icon das Status-Symbol prüfen. Pausierte Synchronisation
wieder anwerfen, Konflikt-Dateien auflösen.

Bei OneDrive: Setze für den Klein.Buch-Backup-Ordner ausdrücklich
"Immer auf diesem Gerät behalten", damit die Dateien nicht nur als
Platzhalter abgelegt werden.

## Wenn nichts hilft

Stoße ein manuelles Backup über "Jetzt Backup machen" an.
Klein.Buch zeigt den Fehler dann in Echtzeit. Notiere die genaue
Fehlermeldung und melde den Fall im Klein.Buch-Repository auf
GitHub. Hänge keine Datei-Inhalte aus Deiner Datenbank an, nur
den Text der Fehlermeldung und die Klein.Buch-Version.

In der Zwischenzeit kannst Du die verschlüsselte Datenbank-Datei
mit dem Datei-Explorer manuell an einen sicheren Ort kopieren.
Den Pfad zum Daten-Ordner findest Du im Hauptmenü unter "Über".

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
