---
slug: troubleshooting-backup
title: Backup-Fehler
category: troubleshooting
order: 520
keywords: [backup, fehler, ziel, schreibrecht, speicher voll, sftp, fingerprint, volume, fehlgeschlagen]
---

# Backup-Fehler

Wenn Klein.Buch ein Backup nicht schreiben kann, steht im
Backup-Protokoll eine Zeile mit "Fehlgeschlagen" und der Fehlermeldung.
Auf der Seite "Backups" steht unter dem betroffenen Ziel
"Fehlgeschlagen" mit Zeitpunkt und Meldung, und unter "Hinweise"
erscheint ein Eintrag.

Wichtig ist, wo der Fehler passiert. Scheitert nur ein externes Ziel,
sind die lokale Kopie und alle anderen Ziele trotzdem gesichert. Der
Hinweis heißt dann "Externe Backup-Spiegelung fehlgeschlagen". Scheitert
dagegen die lokale Kopie im Daten-Ordner des Servers, entsteht gar kein
Backup. Klein.Buch meldet dann, dass die lokale Pflicht-Sicherung nicht
geschrieben werden konnte. Das ist der ernstere Fall.

Die meisten Ursachen liegen beim Server und nicht in Klein.Buch selbst.
Einige Lösungen muss deshalb der Betreiber umsetzen, also die Person, die
den Server und den Container verwaltet.

## "Volume … ist nicht eingebunden"

Diese Meldung (oder "Backup-Ziel-Wurzel /backup-targets fehlt") heißt:
Der Ordner, der als Ziel eingetragen ist, kommt im Container nicht an.
Meist wurde der Container ohne die passende Zeile unter `volumes:` neu
erzeugt, oder der Ordner auf dem Server war beim Containerstart nicht da, etwa
weil das NAS noch nicht verbunden war.

Lösung für den Betreiber: In der Compose-Datei prüfen, ob die Zeile für
das Ziel noch da ist und der Name nach `/backup-targets/` genau gleich
geblieben ist. Klein.Buch merkt sich das Ziel unter diesem Namen. Dann
prüfen, ob der Ordner auf dem Server erreichbar ist, und den Container
neu starten. Beim nächsten Backup versucht Klein.Buch es wieder.

## Keine Schreibrechte

Die Meldung enthält "Permission denied" (Zugriff verweigert). Der
Container läuft unter einem eigenen Benutzer, der in der Compose-Datei
unter `user:` steht. Dieser Benutzer darf im Ziel-Ordner nicht
schreiben.

Lösung für den Betreiber: Dem Benutzer aus `user:` Schreibrechte auf den
Ordner geben, zum Beispiel mit `chown`. Bei einer NAS-Freigabe kommt es
zusätzlich auf die Einstellungen der Freigabe an. Mehr dazu im Kapitel
"[Volumes und Rechte](/help/volumes-und-rechte)".

## Speicher voll

Die Meldung enthält "No space left on device" (kein Platz mehr auf dem
Speicher). Entweder ist das Ziel voll oder der Daten-Ordner auf dem
Server. Ist der Daten-Ordner voll, scheitert schon die lokale Kopie.

Lösung: Platz schaffen. Denk daran, dass Klein.Buch manuelle Backups und
Sicherheits-Backups vor einer Wiederherstellung nie selbst löscht und
auf SFTP-Servern gar nichts löscht. Wie viel Platz Klein.Buch braucht,
steht im Kapitel "[Speicher und Ressourcen](/help/speicherbedarf)".

## SFTP-Server nicht erreichbar oder Anmeldung abgelehnt

Beim Einrichten oder beim Knopf "Testen" lautet die Meldung "Verbindung
zu Host:Port fehlgeschlagen (Host, Port, Zugangsdaten und Firewall
prüfen)." Klein.Buch sagt dabei absichtlich nicht, ob der Server nicht
antwortet, der Port zu ist oder das Passwort nicht stimmt. Beim Backup
selbst steht im Protokoll eine genauere Meldung, zum Beispiel "SFTP-Login
abgelehnt" (Benutzername oder Passwort falsch) oder "hat das Zeitlimit
überschritten".

Mögliche Ursachen:

1. Der SFTP-Server ist ausgeschaltet oder in Wartung.
2. Eine Firewall blockiert die Verbindung. Denk daran: Die Verbindung
   geht vom Server aus, auf dem Klein.Buch läuft, nicht von Deinem Gerät.
3. Adresse oder Port stimmen nicht.
4. Das Passwort wurde beim Anbieter geändert.

Ein SFTP-Ziel lässt sich nicht bearbeiten. Hat sich das Passwort oder
ein anderer Zugangswert geändert, entfernst Du das Ziel mit "Entfernen"
und legst es neu an, wie im Kapitel "[Off-Site-Backup
einrichten](/help/backup-ziel-einrichten)" beschrieben.

Zwei weitere Meldungen kommen nur beim Testen vor. "Port … ist nicht
freigegeben" heißt: Der Port steht nicht auf der Liste der erlaubten
Ports. Der Betreiber kann ihn über die Umgebungsvariable
`KLEIN_BUCH_ALLOWED_TEST_PORTS` freigeben. "Zu viele Verbindungstests"
heißt: Es gab mehr als zehn Tests in einer Minute. Dann eine Minute
warten.

"Zeitüberschreitung nach … s" heißt: Die Übertragung hat zu lange
gedauert. Jedes Ziel hat dafür 60 Sekunden plus eine Sekunde je
Megabyte Backup-Größe. Eine sehr langsame Verbindung reicht dafür nicht
aus.

## SFTP-Host-Schlüssel weicht ab

Die Meldung lautet "SFTP-Host-Schlüssel weicht vom hinterlegten
Fingerprint ab!" und nennt den erwarteten und den erhaltenen Wert.
Klein.Buch hat sich beim Einrichten den Fingerabdruck des Servers
gemerkt. Jetzt meldet sich der Server mit einem anderen. Das ist ein
Sicherheits-Hinweis: Entweder hat der Betreiber des SFTP-Servers den
Schlüssel ausgetauscht, zum Beispiel nach einer Neuinstallation, oder
jemand versucht, sich zwischen Klein.Buch und den Server zu schalten
(ein sogenannter Man-in-the-Middle-Angriff). Klein.Buch bricht die
Übertragung deshalb ab.

Lösung: Frag beim Anbieter nach, ob der Schlüssel ausgetauscht wurde, und
lass Dir den neuen Fingerprint nennen. Wenn ja, entfernst Du das Ziel,
legst es neu an, vergleichst beim "Verbindung testen" den neuen
Fingerprint mit dem genannten und klickst erst dann auf "Als SFTP-Ziel
hinzufügen". Hat der Anbieter nichts ausgetauscht, lege das Ziel nicht
neu an, sondern lass die Sache untersuchen.

## "Kein SFTP-Passwort gespeichert"

Zu diesem Ziel liegt kein Passwort in der Datenbank. Das kommt vor, wenn
ein SFTP-Ziel aus der früheren Desktop-Fassung mit einem Backup
übernommen wurde. Lösung: Ziel entfernen und neu anlegen.

## Sync-Ordner lädt nicht hoch

Das Protokoll meldet "Gesichert", aber im Cloud-Speicher fehlt die
Datei. Klein.Buch legt die Datei nur in den Ordner. Das Hochladen
erledigt ein Sync-Programm auf dem Server, und das läuft gerade nicht
richtig. Lösung für den Betreiber: Das Sync-Programm prüfen.

## Nach einem Update: "Entsperren abgebrochen"

Bei der Anmeldung erscheint "Entsperren abgebrochen: Vor der
Datenbank-Umstellung ließ sich kein Backup schreiben. Es wurde nichts
verändert". Eine neue Version von Klein.Buch muss die Datenbank manchmal
auf ein neues Format umstellen. Vorher legt sie zur Sicherheit ein
Backup an. Klappt das nicht, stellt sie nichts um und bleibt gesperrt.
Meist ist der Daten-Ordner voll oder nicht beschreibbar.

Lösung für den Betreiber: Platz schaffen oder die Rechte von `/data`
korrigieren und die genaue Ursache im Server-Log nachlesen. Danach
meldest Du Dich erneut an. Alternativ kann der Betreiber zur vorherigen
Version zurückkehren, siehe Kapitel "[Updates und
Rückfall](/help/updates-und-rueckfall)".

## Seit einem Neustart entstehen keine Backups

Nach jedem Neustart des Servers ist Klein.Buch gesperrt, bis sich jemand
anmeldet. Solange laufen keine geplanten Aufgaben, auch nicht das
tägliche Backup. Nach der Anmeldung holt Klein.Buch es nach (genau eines,
auch nach mehreren Tagen und auch, wenn Du Dich vor der eingestellten
Uhrzeit anmeldest). Ein Backup nach dem Festschreiben, das wegen "Datenbank
sperren" oder eines Neustarts ausfiel, holt es ebenfalls kurz nach der
Anmeldung nach. Melde Dich nach einem Neustart deshalb bald wieder an; das
Container-Log meldet den gesperrten Zustand alle sechs Stunden.

## Nach einer Wiederherstellung kommt nichts auf den externen Zielen an

Das ist Absicht: Bis Du oben im Hinweis "Automatische Abo-Läufe
angehalten." mit "Prüfen und freigeben" bestätigst, dass dies die
produktive Instanz ist, schreibt Klein.Buch Backups nur in den lokalen
Ordner (siehe "[Backup und
Wiederherstellen](/help/backup-und-wiederherstellen)"). Nach "Jetzt
sichern" stehen die Ziele in der Karte "Backup jetzt erstellen" deshalb
als "Ausgesetzt" mit dem Zusatz "Ausgesetzt bis zur Freigabe nach der
Wiederherstellung (nur lokale Kopie).", und die Meldung "Backup
erstellt: …" kommt als Warnung. Das ist kein Fehler der Ziele. Ist die
letzte externe Kopie älter als sieben Tage, melden Seite "Backups" und
Startseite das ebenfalls; nach der Freigabe bringt das nächste Backup
die Ziele wieder auf den neuesten Stand.

## Wenn nichts hilft

Klick auf "Jetzt sichern". Scheitert schon die lokale Kopie, erscheint
die Meldung "Backup fehlgeschlagen: …" mit dem Grund. Scheitert ein
externes Ziel, steht es in der Karte "Backup jetzt erstellen" als
"Fehlgeschlagen" mit dem Grund daneben.
Der Betreiber findet weitere Einzelheiten im Server-Log, zum Beispiel mit
`docker compose logs klein-buch` (siehe Kapitel "[Fehlersuche im
Container](/help/fehlersuche-im-container)"). Notiere die genaue
Fehlermeldung und die Klein.Buch-Version und melde den Fall im
öffentlichen Klein.Buch-Repository. Den Link findest Du im Hauptmenü
ganz unten unter "Über". Hänge keine Inhalte aus Deiner Datenbank an, nur den Text
der Fehlermeldung und die Version.

In der Zwischenzeit kann der Betreiber die vorhandenen Backup-Dateien
(`.kbk`) aus dem lokalen Backup-Ordner von Hand an einen sicheren Ort
kopieren. Auf dem Server liegt dieser Ordner im Unterordner `backups`
des Ordners, der als `/data` eingebunden ist. Die Dateien sind
verschlüsselt.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
