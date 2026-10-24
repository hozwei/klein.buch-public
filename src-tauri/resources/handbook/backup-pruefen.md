---
slug: backup-pruefen
title: Backup prüfen
category: erste-schritte
order: 80
keywords: [backup, prüfen, protokoll, restore, wiederherstellen, off-site, sicherung, test]
---

# Backup prüfen

Ein Backup, das Du nie geprüft hast, ist im Notfall vielleicht kein
Backup. Klein.Buch macht es Dir leicht, regelmäßig zu kontrollieren, dass
Deine Sicherungen wirklich laufen und sich im Ernstfall zurückspielen
lassen.

## Was Du regelmäßig kontrollierst

1. Läuft das automatische Backup wirklich?
2. Liegt das letzte erfolgreiche Backup auf einem externen Ziel nicht zu
   lange zurück?
3. Lässt sich ein Backup tatsächlich wiederherstellen?

Die ersten beiden Punkte prüfst Du einmal pro Woche mit einem Blick ins
Protokoll. Den dritten prüfst Du seltener, etwa alle drei Monate, mit
einem Probe-Wiederherstellen.

## Das Backup-Protokoll lesen

Klick im Hauptmenü auf "Einstellungen", dann auf "Backups" und oben
rechts auf "Backup-Protokoll". Das Protokoll ist der lückenlose und
unveränderliche Nachweis jeder Sicherung, auch der fehlgeschlagenen.
Jede Zeile nennt den Zeitpunkt, den Status ("gesichert" oder
"fehlgeschlagen"), den Auslöser ("Manuell", "Täglich (automatisch)",
"Beleg festgeschrieben" oder "Vor Wiederherstellung"), das Ziel ("Lokal
(Pflichtkopie)", "Ordner (extern)" oder "SFTP-Server", jeweils mit
Deiner Bezeichnung), die Datei und die Größe. Jeder Backup-Lauf erzeugt
eine Zeile für die lokale Kopie und eine Zeile je externem Ziel.

Über die Felder "Suche", "Von", "Bis", "Status", "Auslöser" und "Ziel"
grenzt Du die Liste ein, mit "Suchen" wendest Du die Filter an,
"Zurücksetzen" hebt sie wieder auf.

Schau nach der neuesten Zeile je Ziel. Steht dort "gesichert", ist alles
in Ordnung. Ist das letzte erfolgreiche Backup auf einem externen Ziel
älter als sieben Tage, läuft etwas nicht rund. Klein.Buch meldet das
ohnehin unter "Hinweise" mit dem Eintrag "Off-Site-Backup überfällig".
Einen schnellen Überblick gibt auch die Seite "Backups": Unter jedem
externen Ziel steht, wann es zuletzt geklappt hat oder warum nicht.

## Wenn ein Eintrag "fehlgeschlagen" zeigt

Die Fehlermeldung steht in derselben Zeile direkt unter dem Dateinamen.
Die häufigsten Ursachen sind:

1. Der Ordner eines externen Ziels ist nicht mehr eingebunden oder das
   NAS dahinter ist ausgeschaltet.
2. Der Benutzer, unter dem der Container läuft, darf im Ziel-Ordner
   nicht schreiben.
3. Der Speicher ist voll, am Ziel oder auf dem Server.
4. Der SFTP-Server ist nicht erreichbar, lehnt die Anmeldung ab oder
   meldet sich mit einem anderen Schlüssel als beim Einrichten.

Behebe die Ursache und klick auf der Seite "Backups" auf "Jetzt sichern".
Klein.Buch protokolliert das Ergebnis erneut. Wenn der Fehler bleibt,
schau ins Kapitel "[Backup-Fehler](/help/troubleshooting-backup)".

Eine Sache kann das Protokoll nicht sehen: Lädt ein Sync-Programm auf
dem Server die Dateien aus einem Ordner-Ziel in einen Cloud-Speicher
hoch, meldet Klein.Buch "gesichert", sobald die Datei im Ordner liegt.
Ob das Hochladen danach klappt, prüfst Du beim Cloud-Speicher selbst.

## Probe-Wiederherstellen (alle drei Monate)

Beim Probe-Wiederherstellen spielst Du ein Backup in eine getrennte
Test-Instanz ein, ohne Deine echte Instanz anzufassen. Eine Instanz ist
eine eigenständige Klein.Buch-Installation mit eigenen Daten. In Deiner
echten Instanz probierst Du das bitte nicht aus: Dort würde die
Wiederherstellung alles ersetzen, was Du seit dem Backup erfasst hast.

So gehst Du vor:

1. Der Betreiber startet einen zweiten Klein.Buch-Container mit eigenen,
   leeren Ordnern für `/data` und `/inputs` und mit einer eigenen
   Adresse, wie im Kapitel "[Installation mit Docker Compose](/help/installation-docker)"
   beschrieben. Die Backup-Ziele der echten Instanz bindet er dort nicht
   ein. Sonst würde die Test-Instanz in diese Ordner schreiben und dort
   auch alte Backups aussortieren.
2. Du richtest die Test-Instanz ganz normal ein, mit einer beliebigen
   Test-Passphrase und einem eigenen Eintrag in der Authenticator-App.
3. Unter "Einstellungen" und "Backups" lädst Du in der Karte
   "Wiederherstellen" das neueste Backup von Deinem externen Ziel hoch,
   gibst als "Backup-Passwort" Deine echte Passphrase ein und klickst auf
   "Wiederherstellen".
4. Erscheint "Wiederherstellung vorbereitet", hat Klein.Buch das Backup
   mit Deiner Passphrase entschlüsselt, auf Vollständigkeit und
   Beschädigung geprüft und die Datenbank darin geöffnet. Das ist der
   wichtigste Teil der Probe: Dein Backup ist lesbar.

Danach startet die Test-Instanz neu und enthält alle Deine Daten und
Einstellungen. Dazu gehören auch die Mail-Konten mit Passwort, die
SFTP-Ziele und Deine Abo-Rechnungen. Sobald Du Dich dort anmeldest,
beginnt Klein.Buch sofort mit seinen geplanten Aufgaben. Fällige
Abo-Rechnungen würden festgeschrieben und, je nach Einstellung, per
E-Mail an Deine Kunden verschickt, und Backups gingen an Deine SFTP-Ziele.
Melde Dich in der Test-Instanz deshalb nur an, wenn sie keine Verbindung
nach außen hat (also weder Mail-Server noch SFTP-Server erreicht). Sonst
hörst Du nach Schritt 4 auf: Der Betreiber hält die Test-Instanz an und
löscht ihre Ordner.

Klappt die Wiederherstellung nicht, lies das Kapitel "[Fehler beim
Wiederherstellen](/help/troubleshooting-restore)".

## Was im Backup steckt

Ein Klein.Buch-Backup enthält:

1. Die verschlüsselte Datenbank mit allen Belegen, Kontakten und
   Einstellungen, auch den gespeicherten Passwörtern für Mail-Konten und
   SFTP-Ziele und der Liste Deiner Backup-Ziele
2. Das komplette Beleg-Archiv (Rechnungs-PDFs, XML-Dateien, Belege,
   Anhänge)
3. Dein Logo und Deine Unterschrift

Es enthält absichtlich nicht:

1. Deine Passphrase (es gibt keine Hintertür)
2. Die Verbindung zu Deiner Authenticator-App (sie liegt getrennt auf dem
   Server)
3. Den Vorlagen-Ordner `/inputs` mit Rechnungs-Layouts, Mail-Vorlagen und
   AfA-Tabellen (AfA steht für Absetzung für Abnutzung, also die
   Abschreibungs-Tabellen). Diesen Ordner sichert der Betreiber selbst.
4. Das Programm Klein.Buch (es kommt aus dem Docker-Image)

Ohne Deine Passphrase ist das Backup wertlos. Notiere sie im
Passwort-Manager. Verlierst Du Passphrase und Server zusammen, sind die
Daten endgültig weg.

## Erinnerung

Klein.Buch erinnert Dich unter "Hinweise", wenn seit mehr als sieben
Tagen kein Backup auf einem externen Ziel geklappt hat (ohne externes
Ziel: wenn seit sieben Tagen gar kein Backup mehr entstanden ist). Die
Erinnerung heißt "Backup überfällig". Du schaltest sie unter "Hinweise"
und "Erinnerungen einstellen" an oder aus. Die Sieben-Tage-Grenze ist
fest.

Hakst Du den Hinweis ab, kommt er am nächsten Tag wieder, solange das
Problem besteht. Zusätzlich meldet Klein.Buch jeden fehlgeschlagenen
externen Kopiervorgang mit dem Hinweis "Externe Backup-Spiegelung
fehlgeschlagen", höchstens einmal pro Tag und Ziel. Diesen Hinweis kannst
Du nicht abschalten.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
