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
Jede Zeile nennt den Zeitpunkt, den Status ("Gesichert", "Gesichert,
unvollständig" oder "Fehlgeschlagen"), den Auslöser ("Manuell", "Täglich (automatisch)",
"Beleg festgeschrieben" oder "Vor Wiederherstellung"), das Ziel ("Lokal
(Pflichtkopie)", "Ordner (extern)" oder "SFTP-Server", jeweils mit
Deiner Bezeichnung), die Datei und die Größe. Jeder Backup-Lauf erzeugt
eine Zeile für die lokale Kopie und eine Zeile je externem Ziel.

Über die Felder "Suche", "Von", "Bis", "Status", "Auslöser" und "Ziel"
grenzt Du die Liste ein. Die Auswahlfelder wirken sofort, Suchtext und
Datum wendest Du mit "Filtern" an; "Filter zurücksetzen" hebt alle
Filter wieder auf. Über der Tabelle steht, wie viele Einträge gefunden
wurden, mit dem Zusatz "(gefiltert)", solange ein Filter aktiv ist.
Angezeigt werden höchstens 1000 Einträge; bei mehr grenzt Du die Suche
ein, um ältere zu sehen.

Schau nach der neuesten Zeile je Ziel. Steht dort "Gesichert", ist alles
in Ordnung. Ist das letzte erfolgreiche Backup auf einem externen Ziel
älter als sieben Tage, läuft etwas nicht rund. Klein.Buch meldet das
ohnehin unter "Hinweise" mit dem Eintrag "Off-Site-Backup überfällig".
Einen schnellen Überblick gibt auch die Seite "Backups": Unter jedem
externen Ziel steht, wann es zuletzt geklappt hat oder warum nicht, und
die Karte "Status" zeigt unter "Stand extern" das Alter der externen
Kopien. Dasselbe Alter steht auf der Startseite in der Kennzahl
"Backup".

## Wenn ein Eintrag "Fehlgeschlagen" zeigt

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

Steht bei einem Eintrag "Gesichert, unvollständig" und darunter
"Unvollständig: … nicht gesichert",
fehlt im Backup etwas aus dem Beleg-Archiv. Klein.Buch sichert nur echte
Dateien. Hat jemand auf dem Server einen Ordner im Archiv durch eine
Verknüpfung (einen symbolischen Link) ersetzt, etwa um alte Jahrgänge
auszulagern, steht er hier und fehlt im Backup. Der Betreiber holt den
Ordner dann zurück oder bindet ihn als Volume ein. Fehlt eine
Archivdatei, klärt die Integritätsprüfung, welche es ist. Klein.Buch
meldet das zusätzlich unter "Hinweise" ("Backup unvollständig").

Eine Sache kann das Protokoll nicht sehen: Lädt ein Sync-Programm auf
dem Server die Dateien aus einem Ordner-Ziel in einen Cloud-Speicher
hoch, meldet Klein.Buch "Gesichert", sobald die Datei im Ordner liegt.
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
SFTP-Ziele und Deine Abo-Rechnungen. Nach der Anmeldung bleiben die
automatischen Abläufe angehalten: Klein.Buch schreibt keine
Abo-Rechnungen fest, verschickt nichts und kopiert Backups nicht auf die
externen Ziele (Hinweis "Automatische Abo-Läufe angehalten" oben). In
der Test-Instanz klickst Du **nie** auf "Prüfen und freigeben" — das ist
nur für die produktive Instanz. So kannst Du Dich dort gefahrlos anmelden,
stichprobenartig Rechnungen öffnen und die Archiv-Prüfung starten.
Danach hält der Betreiber die Test-Instanz an und löscht ihre Ordner.
Ganz ohne Verbindung nach außen (weder Mail-Server noch SFTP-Server
erreichbar) ist die Test-Instanz trotzdem am sichersten.

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

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
