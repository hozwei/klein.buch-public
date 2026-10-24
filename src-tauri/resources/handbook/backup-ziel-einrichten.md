---
slug: backup-ziel-einrichten
title: Off-Site-Backup einrichten
category: erste-schritte
order: 40
keywords: [backup, off-site, externes ziel, sftp, nas, backup-targets, fingerabdruck, sicherung]
---

# Off-Site-Backup einrichten

Klein.Buch schreibt nach jeder Festschreibung und einmal am Tag ein
verschlüsseltes Backup. Eine Kopie landet immer im lokalen Backup-Ordner
auf dem Server. Die richtest Du nicht ein, sie ist einfach da. Sie liegt
aber auf demselben Speicher wie Deine Daten. Fällt die Festplatte aus,
brennt der Raum ab oder wird der Server gestohlen, sind Daten und lokale
Backups zusammen weg.

Deshalb braucht jede Klein.Buch-Instanz mindestens ein Off-Site-Ziel.
Off-Site heißt "nicht am selben Ort": eine Kopie außerhalb des Servers.
In Klein.Buch heißen diese Ziele "Externe Ziele". Du kannst mehrere
gleichzeitig haben (bis zu zehn). Jedes Backup geht an jedes Ziel.
Scheitert eines, sind die lokale Kopie und die übrigen Ziele trotzdem
gesichert.

Klein.Buch hält Dich nicht vom Arbeiten ab, solange kein externes Ziel
eingerichtet ist. Du kannst weiter Rechnungen festschreiben. Aber es
erinnert Dich deutlich: Die Seite "Backups" zeigt eine Warnung, die
Startseite zeigt die Aufgabe "Kein externes Backup-Ziel eingerichtet."
mit dem Knopf "Ziele prüfen", und einmal pro Woche erscheint unter
"Hinweise" der Eintrag "Kein externes Backup-Ziel".

Es gibt zwei Arten von Zielen: einen Ordner, den der Betreiber in den
Container einbindet, und einen SFTP-Server. Der Betreiber ist die Person,
die den Server und den Klein.Buch-Container verwaltet. Das kannst Du
selbst sein oder jemand, der das für Dich macht.

## Ordner-Ziel: Schritt 1 beim Betreiber

Klein.Buch läuft in einem Container (einer abgeschotteten Umgebung auf
dem Server) und sieht vom Server nur die Ordner, die der Betreiber
ausdrücklich hineinreicht. Für Backup-Ziele sind das Ordner unter
`/backup-targets`. Andere Ordner des Servers kann Klein.Buch absichtlich
nicht beschreiben.

Was hinter so einem Ordner steckt, entscheidet der Betreiber: eine
Freigabe auf einem NAS (einem Netzwerk-Speicher), eine zweite
Festplatte oder der Ordner eines Sync-Programms, das die Dateien selbst
in einen Cloud-Speicher hochlädt. Klein.Buch hat keine eigene Verbindung
zu Cloud-Diensten. Es legt die verschlüsselte Datei in den Ordner, den
Rest erledigt das Sync-Programm auf dem Server.

In der Compose-Datei (der Datei, die beschreibt, wie der Container
gestartet wird) kommt für jedes Ziel eine Zeile unter `volumes:` dazu,
zum Beispiel:

```
      - /mnt/nas/backups:/backup-targets/nas
```

Links steht der Ordner auf dem Server, rechts der Name, unter dem
Klein.Buch ihn sieht. Den Teil nach `/backup-targets/` darf der
Betreiber frei wählen. Er sollte ihn später nicht mehr ändern, weil
Klein.Buch sich das Ziel unter diesem Namen merkt. Der Ordner muss für
den Benutzer beschreibbar sein, unter dem der Container läuft (die
Angabe `user:` in der Compose-Datei). Danach den Container neu erzeugen
lassen, zum Beispiel mit `docker compose up -d`. Für Backup-Ziele ist
eine Netzwerk-Freigabe in Ordnung, anders als für den Daten-Ordner
`/data`. Mehr dazu im Kapitel "[Volumes und Rechte](/help/volumes-und-rechte)".

## Ordner-Ziel: Schritt 2 in Klein.Buch

Öffne "Einstellungen" und "Backups". In der Karte "Externe Ziele" gibt
es den Abschnitt "Ordner-Ziel hinzufügen" mit drei Feldern:

1. "Eingebundener Ordner": Hier stehen alle Ordner, die der Betreiber
   unter `/backup-targets` eingebunden hat, im Beispiel oben "nas".
2. "Unterordner": Vorbelegt ist "klein-buch". Klein.Buch legt diesen
   Unterordner selbst an.
3. "Bezeichnung": Ein Name, an dem Du das Ziel später wiedererkennst,
   zum Beispiel "NAS im Büro".

Dann klickst Du auf "Ordner-Ziel hinzufügen". Klein.Buch prüft mit einer
Testdatei, ob es dort schreiben darf. Eventuell fragt es im Fenster
"Aktion bestätigen" noch einmal nach Daten-Passwort und App-Code. Ab dem
nächsten Backup wird das Ziel beschrieben.

Steht dort statt der Felder der Hinweis "Unter /backup-targets ist kein
Ordner eingebunden", fehlt Schritt 1.

## SFTP-Ziel

SFTP (SSH File Transfer Protocol) ist ein Verfahren, um Dateien
verschlüsselt auf einen anderen Rechner zu kopieren. Das kann ein
zweites Gerät in Deinem Netz sein oder ein Anbieter, der Speicherplatz
mit SFTP-Zugang vermietet. Du brauchst von ihm die Server-Adresse (zum
Beispiel `backup.example.org`), den Port, den Benutzernamen, den
Zielordner und das Passwort. Klein.Buch meldet sich bei SFTP-Servern
nur mit Passwort an.

Für den Verbindungstest, ohne den sich kein SFTP-Ziel anlegen lässt, sind
ohne weitere Einstellung nur die Ports 22 und 2222 erlaubt. Ein Port ist
eine Art Türnummer am Server, SFTP nutzt meistens die 22. Braucht Dein
Server einen anderen Port, trägt der
Betreiber die erlaubten Ports in die Umgebungsvariable
`KLEIN_BUCH_ALLOWED_TEST_PORTS` ein, durch Komma getrennt, und startet
den Container neu. Achtung: Diese Liste ersetzt die Standard-Ports für
SFTP und zugleich für den Mail-Versand (SMTP). Sie muss also alle Ports
enthalten, die Du brauchst, zum Beispiel `22,587,2200`.

So richtest Du das Ziel ein:

1. In der Karte "SFTP-Ziel hinzufügen (für Fortgeschrittene)" füllst Du
   "Bezeichnung", "Server (Host)", "Port", "Benutzer", "Zielordner auf
   dem Server" (leer bedeutet: das Home-Verzeichnis des Benutzers) und
   "Passwort" aus.
2. Klick auf "Verbindung testen". Klein.Buch verbindet sich, versucht
   eine Testdatei zu schreiben und zeigt das Ergebnis ("Schreibtest im
   Zielordner: ok") und den Host-Fingerprint.
3. Vergleiche den Host-Fingerprint mit dem Wert, den Dir der Anbieter
   genannt hat. Der Fingerprint ist eine Art Fingerabdruck des Servers.
   Er beginnt mit `SHA256:`. Wenn Du den Server selbst betreibst, zeigt
   der Befehl `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` auf dem
   Server den Wert an. Ein Server hat oft mehrere solche Schlüssel (die
   anderen Dateien in `/etc/ssh` mit `_key.pub` am Ende). Der angezeigte
   Fingerprint muss mit einem davon Zeichen für Zeichen übereinstimmen.
4. Stimmt er, klickst Du auf "Als SFTP-Ziel hinzufügen". Klein.Buch merkt
   sich den Fingerprint und verweigert ab dann jede Verbindung zu einem
   Server, der sich mit einem anderen Schlüssel meldet. So kann Dich
   niemand unbemerkt auf einen falschen Server umleiten.

Zwischen Test und Hinzufügen dürfen höchstens zehn Minuten liegen, und
Server und Port müssen gleich bleiben. Änderst Du einen davon, verlangt
Klein.Buch einen neuen Test. Insgesamt sind höchstens zehn
Verbindungstests pro Minute möglich.

Das Passwort speichert Klein.Buch verschlüsselt in der Datenbank dieser
Instanz, nur für dieses eine Ziel. Weil es in der Datenbank liegt, ist
es auch in jedem Backup enthalten. Ein SFTP-Ziel lässt sich nicht
nachträglich ändern. Für einen anderen Server, einen anderen Benutzer,
ein neues Passwort oder einen neuen Fingerprint entfernst Du das Ziel und
legst es neu an. In der Liste der Ziele hat jedes SFTP-Ziel den Knopf
"Testen". Er prüft die Verbindung mit dem gespeicherten Passwort und dem
gemerkten Fingerprint.

Alte Backups löscht Klein.Buch auf einem SFTP-Server bewusst nicht: Eine
kompromittierte Instanz soll die externen Kopien nicht vernichten können.
Dort räumst Du oder der Betreiber des Servers selbst auf, etwa mit
Snapshots oder einem zeitgesteuerten Aufräumen nach dem Muster "30 Tage
vollständig, danach eines je Monat" (Platzbedarf siehe "[Speicher und
Ressourcen](/help/speicherbedarf)"). Während Klein.Buch nach einer
Wiederherstellung angehalten ist, schreibt es nichts auf die externen
Ziele.

## Ziel entfernen

Neben jedem Ziel steht "Entfernen". Nach der Rückfrage gehen künftige
Backups nicht mehr dorthin. Backups, die schon am Ziel liegen, bleiben
liegen.

## Prüfen, ob es läuft

Klick in der Karte "Backup jetzt erstellen" auf "Jetzt sichern".
Klein.Buch meldet danach je Ziel, ob die Kopie geklappt hat. In der
Liste der Ziele steht unter jedem Ziel "Zuletzt erfolgreich" oder
"Fehlgeschlagen" mit Zeitpunkt und Meldung. Jeder einzelne Versuch steht
außerdem im "Backup-Protokoll" oben rechts. Wie Du es liest, erklärt das
Kapitel "[Backup prüfen](/help/backup-pruefen)". Bei Fehlern hilft das
Kapitel "[Backup-Fehler](/help/troubleshooting-backup)".

## Was am Ziel liegt

Am Ziel liegen Dateien mit Namen wie
`klein-buch-20261024-143005123-1a2b3c4d.kbk`. Der Name enthält Datum und
Uhrzeit der Sicherung. Jede Datei ist mit Deiner Passphrase verschlüsselt
und ohne sie wertlos, auch für jeden, der Zugriff auf das NAS oder den
Cloud-Speicher hat. Klein.Buch schreibt jede Datei zuerst unter einem
vorläufigen Namen und benennt sie erst um, wenn sie vollständig ist. Eine
halb geschriebene Datei trägt deshalb nie den endgültigen Namen.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
