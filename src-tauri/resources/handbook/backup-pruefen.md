---
slug: backup-pruefen
title: Backup prüfen
category: erste-schritte
order: 80
keywords: [backup, prüfen, protokoll, restore, wiederherstellen, off-site, sicherung]
---

# Backup prüfen

Ein Backup, das Du nie geprüft hast, ist im Notfall vielleicht
kein Backup. Klein.Buch macht es Dir leicht, regelmäßig zu
kontrollieren, dass Deine Sicherungen wirklich laufen und sich im
Ernstfall wiederherstellen lassen.

## Was Du regelmäßig kontrollierst

1. Läuft das automatische Backup wirklich?
2. Liegt das letzte erfolgreiche Off-Site-Backup nicht zu lange
   zurück?
3. Lässt sich ein Backup tatsächlich wiederherstellen?

Den ersten und zweiten Punkt prüfst Du jede Woche mit einem Blick
ins Protokoll. Den dritten Punkt prüfst Du seltener (etwa alle drei
Monate) mit einem echten Probe-Wiederherstellen.

## Backup-Protokoll lesen

Klick im Hauptmenü auf "Einstellungen", dann auf "Backups". Oben
rechts in der PageBar gibt es den Knopf "Backup-Protokoll". Die
Liste zeigt jedes erzeugte Backup mit Erstellt-Datum, Grund (zum
Beispiel "Nach dem Festschreiben", "Automatisch (täglich)",
"Manuell", "Vor einer Wiederherstellung"), Ort, Aufbewahrung,
Größe und Status.

Suche nach den letzten Einträgen pro Ziel. Wenn das letzte
Off-Site-Backup älter als sieben Tage ist, läuft etwas nicht
rund. Klein.Buch warnt Dich in diesem Fall ohnehin über den
Hinweise-Bereich.

## Wenn ein Backup-Eintrag "Fehler" zeigt

Klick auf den Eintrag. Klein.Buch zeigt Dir die genaue
Fehlermeldung des Betriebssystems oder des SFTP-Servers. Die
häufigsten Ursachen:

1. Cloud-Ordner ist nicht synchronisiert oder offline
2. Externer USB-Stick ist nicht angesteckt
3. SFTP-Server nicht erreichbar (Netzwerk weg, Server-Wartung)
4. Schreibrechte auf dem Ziel-Ordner entzogen
5. Festplatte voll

Behebe die Ursache und stoße über "Jetzt Backup machen" einen
neuen Versuch an. Klein.Buch loggt das Ergebnis erneut. Wenn der
Fehler bleibt, schau ins Kapitel "Backup-Fehler" in der Kategorie
"Troubleshooting".

## Probe-Wiederherstellen (alle drei Monate)

Ein Probe-Wiederherstellen spielt ein Backup in einer separaten
Test-Umgebung zurück, ohne Deine echte Datenbank zu verändern. So
weißt Du, dass das Backup wirklich funktioniert.

Die einfache Variante: Auf einem zweiten Rechner Klein.Buch
installieren, das Backup hineinkopieren und die Wiederherstellung
ausführen. Klein.Buch fragt nach der Passphrase. Wenn die
Wiederherstellung durchläuft und Du Deine Daten siehst, ist das
Backup gesund.

Die kompakte Variante auf demselben Rechner: Vor der
Wiederherstellung zieht Klein.Buch automatisch ein
Sicherheits-Backup Deiner aktuellen Datenbank. Du kannst die
Wiederherstellung deshalb gefahrlos ausprobieren und im Anschluss
über das Backup-Protokoll wieder zurück auf den vorherigen Stand.

Wenn die Wiederherstellung fehlschlägt, lies das Kapitel "Fehler
beim Wiederherstellen" in der Kategorie "Troubleshooting".

## Was im Backup steckt

Ein Klein.Buch-Backup enthält:

1. Die verschlüsselte Datenbank
2. Das komplette Beleg-Archiv (PDF, XML, Quittungen, Logos)
3. Das Verkäuferprofil und die Einstellungen

Es enthält absichtlich nicht:

1. Deine Passphrase (kein Recovery-Backdoor)
2. OAuth- und SMTP-Anmeldedaten (liegen im Schlüsselbund Deines
   Betriebssystems, nicht in der Datenbank)
3. Die Klein.Buch-Programm-Dateien selbst (kommen vom Installer)

Ohne Deine Passphrase ist das Backup wertlos. Notiere die
Passphrase im Passwort-Manager. Wenn Du die Passphrase und das
Backup zusammen verlierst, sind die Daten endgültig weg.

## Erinnerung einrichten

Klein.Buch zeigt Dir im Bereich "Hinweise" automatisch eine
Erinnerung, wenn ein Off-Site-Backup zu lange her ist. Die
Schwelle (zum Beispiel sieben Tage) stellst Du in den
Einstellungen ein.

Wenn Du die Hinweise leerst, kommt die Erinnerung beim nächsten
Überschreiten der Schwelle wieder. Sie verschwindet nicht
dauerhaft.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
