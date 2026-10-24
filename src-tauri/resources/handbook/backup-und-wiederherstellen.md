---
slug: backup-und-wiederherstellen
title: Backup und Wiederherstellen
category: bedienen
order: 220
keywords: [backup, restore, wiederherstellen, off-site, sftp, aufbewahrung, protokoll, kbk]
---

# Backup und Wiederherstellen

Ein Backup ist eine verschlüsselte Sicherheits-Kopie Deiner Datenbank und
Deines Beleg-Archivs. Klein.Buch legt es automatisch an: nach jeder
Festschreibung (zum Beispiel wenn Du eine Rechnung ausstellst, eine
Ausgabe oder Privatbewegung festschreibst oder etwas stornierst) und
zusätzlich einmal am Tag. Du brauchst Dich im Alltag nicht darum zu
kümmern. Wenn doch einmal ein älterer Stand zurück muss, gibt es die
Wiederherstellung (auf Englisch "Restore").

Das tägliche Backup läuft nur, solange Klein.Buch entsperrt ist. Nach
einem Neustart des Servers ist Klein.Buch gesperrt, bis Du Dich wieder
anmeldest. Erst dann holt es das fällige Backup nach.

## Wo die Backups liegen

Jedes Backup landet zuerst im lokalen Backup-Ordner auf dem Server
(`/data/backups`). Diese lokale Sicherung läuft immer und lässt sich
nicht abschalten. Sie liegt aber auf demselben Speicher wie Deine Daten.
Geht der Server kaputt, hilft sie Dir nicht. Deshalb schickt Klein.Buch
jedes Backup zusätzlich an jedes externe Ziel, das Du eingerichtet hast
(Off-Site-Backup, also eine Kopie außerhalb des Servers). Wie das geht,
steht im Kapitel "[Off-Site-Backup einrichten](/help/backup-ziel-einrichten)".

Alles rund ums Backup findest Du im Hauptmenü unter "Einstellungen" und
dort unter "Backups". Oben siehst Du den Status, darunter die externen
Ziele, den Knopf für ein sofortiges Backup, den Backup-Verlauf und ganz
unten die Wiederherstellung. Oben rechts führt "Backup-Protokoll" zum
lückenlosen Nachweis aller Sicherungen.

## Der Backup-Verlauf

Die Tabelle "Backup-Verlauf" zeigt die Sicherungen, die Du zurückspielen
kannst, die neueste zuerst. Jede Zeile nennt, wann das Backup erstellt
wurde, den Grund ("Nach dem Festschreiben", "Automatisch (täglich)",
"Manuell" oder "Vor einer Wiederherstellung"), den Ort ("Lokal",
"Off-Site" oder "Off-Site (SFTP)"), die Aufbewahrungs-Klasse und die
Größe.

Der Verlauf zeigt nur Sicherungen, die geklappt haben. Fehlgeschlagene
Versuche, etwa eine externe Kopie, die nicht geschrieben werden konnte,
stehen im "Backup-Protokoll". Wie Du es liest, erklärt das Kapitel
"[Backup prüfen](/help/backup-pruefen)".

## Aufbewahrung

Klein.Buch hebt nicht jedes Backup für immer auf, sondern sortiert ältere
automatisch aus. Es unterscheidet dabei drei Klassen: Monats-Backups
(alle, die an einem Monatsersten entstehen), Jahres-Backups (alle vom
1. Januar) und alle übrigen.

Auf jedem externen Ziel behält Klein.Buch die 30 neuesten gewöhnlichen
Backups, die 12 neuesten Monats-Backups und die 7 neuesten
Jahres-Backups. Im lokalen Backup-Ordner sind es nur die 7 neuesten
gewöhnlichen, die 3 neuesten Monats-Backups und das neueste
Jahres-Backup. Die kurze lokale Reihe gilt aber nur, solange mindestens
ein externes Ziel beim letzten Mal erfolgreich war. Gibt es kein
funktionierendes externes Ziel, ist der lokale Ordner Deine einzige Kopie
und behält deshalb die lange Reihe.

Gezählt werden Backups, nicht Tage. Schreibst Du an einem Tag zehn
Rechnungen fest, sind die sieben neuesten lokalen Backups alle von diesem
Tag. Die längere Reihe liegt auf den externen Zielen.

Nie automatisch gelöscht werden manuelle Backups ("Jetzt sichern") und
die Sicherheits-Backups vor einer Wiederherstellung. Dasselbe gilt für
die Backups, die Klein.Buch vor einer Datenbank-Umstellung nach einem
Update und nach einem Wechsel des Daten-Passworts anlegt. Sie erscheinen
im Verlauf als "Manuell". Diese Backups sammeln sich mit der Zeit an.
Plane den Platz dafür ein (siehe "[Speicher und Ressourcen](/help/speicherbedarf)").

Auf SFTP-Servern entfernt Klein.Buch alte Backups nur aus seiner eigenen
Liste. Die Dateien auf dem Server selbst löscht es nicht. Dort räumst Du
oder der Betreiber des Servers von Zeit zu Zeit selbst auf.

## Manuelles Backup auslösen

In der Karte "Backup jetzt erstellen" klickst Du auf "Jetzt sichern".
Klein.Buch erzeugt sofort ein Backup, schreibt es lokal und auf alle
externen Ziele und meldet das Ergebnis: Dateiname und Größe, dazu je
externes Ziel, ob die Kopie geklappt hat. Der Knopf funktioniert nur,
wenn Klein.Buch entsperrt ist.

Ein manuelles Backup lohnt sich vor größeren Änderungen, zum Beispiel
bevor Du Deine Firmendaten umbaust oder bevor der Betreiber ein Update
einspielt.

## Wiederherstellen

Eine Wiederherstellung ersetzt den gesamten aktuellen Stand durch den
Stand aus dem Backup. Alles, was Du seit diesem Backup erfasst hast, ist
danach nicht mehr in Klein.Buch. Deshalb legt Klein.Buch vorher
automatisch ein Sicherheits-Backup des aktuellen Stands an.

Als Quelle gibt es zwei Wege. Entweder wählst Du ein Backup aus dem
lokalen Backup-Ordner direkt in der Liste. Oder Du lädst eine
Backup-Datei hoch. Klein.Buch-Backups haben die Endung `.kbk`. Backups,
die auf einem externen Ziel liegen, sind in der Liste mit "erst laden"
markiert: Kopiere die Datei zuerst vom NAS, aus dem Sync-Ordner oder vom
SFTP-Server auf Dein Gerät und lade sie dann hoch.

So läuft es ab:

1. Öffne "Einstellungen" und "Backups". Klick im "Backup-Verlauf" neben
   dem gewünschten Backup auf "Auswählen". Oder wähle unten in der Karte
   "Wiederherstellen" im Feld "Backup-Datei" eine `.kbk`-Datei von
   Deinem Gerät.
2. Gib im Feld "Backup-Passwort" die Passphrase ein, die galt, als dieses
   Backup erstellt wurde.
3. Klick auf "Wiederherstellen" und bestätige die Rückfrage "Backup
   wiederherstellen?". Liegt Deine letzte Bestätigung länger als fünf
   Minuten zurück, fragt Klein.Buch im Fenster "Aktion bestätigen" noch
   einmal nach Daten-Passwort und Code aus der Authenticator-App.
4. Bei einer hochgeladenen Datei siehst Du den Fortschritt ("Hochladen …").
   Danach prüft Klein.Buch das Backup: stimmt die Passphrase, ist die
   Datei vollständig, passt die Version. Erst dann legt es das
   Sicherheits-Backup an und bereitet die Wiederherstellung vor.
5. Klein.Buch meldet "Wiederherstellung vorbereitet" mit dem Namen des
   Sicherheits-Backups. Der Server startet jetzt neu und spielt das
   Backup beim Start ein. Bis dahin nimmt er keine Änderungen an. Warte,
   bis die Anmeldung wieder erscheint.
6. Melde Dich an, und zwar mit der Passphrase des eingespielten Backups
   und dem Code aus Deiner Authenticator-App.

Für Schritt 5 muss der Container so eingerichtet sein, dass er nach dem
Beenden von selbst wieder startet (im Compose-Beispiel
`restart: unless-stopped`). Ist der Server nach drei Minuten nicht
zurück, sagt Dir die Seite das. Was dann zu tun ist, steht im Kapitel
"[Fehler beim Wiederherstellen](/help/troubleshooting-restore)".

Eingespielt werden die Datenbank, das Beleg-Archiv sowie Logo und
Unterschrift. In der Datenbank stecken auch die gespeicherten Passwörter
für Mail-Konten und SFTP-Ziele und die Liste Deiner Backup-Ziele. Sie
kommen also auf den Stand des Backups. Nicht Teil des Backups sind die
Verbindung zu Deiner Authenticator-App (sie bleibt, wie sie ist) und der
Vorlagen-Ordner `/inputs` mit den Rechnungs-Layouts. Fehlt nach der
Wiederherstellung eine Vorlage, die Deine Daten benutzen, meldet
Klein.Buch das unter "Hinweise".

Klein.Buch nimmt Backups ab Version 2026.5 an. Stammt ein Backup aus
einer älteren Version als der laufenden, stellt Klein.Buch die Datenbank
bei der nächsten Anmeldung um und legt vorher selbst ein Backup an.
Backups aus einer neueren Version lehnt es ab. Dann muss zuerst der
Betreiber Klein.Buch aktualisieren. Backups der früheren Desktop-Fassung
lassen sich nur einspielen, solange ihr Beleg-Archiv leer ist. Enthalten
sie festgeschriebene Belege, lehnt Klein.Buch sie ab, denn einen Umzug
von der Desktop-Fassung in den Server gibt es nicht.

## Zurück zum Stand vor der Wiederherstellung

Hat die Wiederherstellung nicht das Erwartete gebracht, zum Beispiel weil
Du das falsche Backup gewählt hast, spielst Du einfach das
Sicherheits-Backup zurück. Es steht im Verlauf mit dem Grund "Vor einer
Wiederherstellung" und wird auch auf Deine externen Ziele kopiert. Es
ist mit dem Daten-Passwort verschlüsselt, das vor der Wiederherstellung
galt. Du wählst es mit "Auswählen" aus und gehst genauso vor wie oben.

## Passphrase und ältere Backups

Jedes Backup ist mit der Passphrase verschlüsselt, die bei seiner
Erstellung galt. In Klein.Buch heißt sie an manchen Stellen auch
"Daten-Passwort" oder "Backup-Passwort", gemeint ist immer dasselbe.
Änderst Du sie unter "Einstellungen" und "Sicherheit & Anmeldung",
verschlüsselt Klein.Buch neue Backups mit der neuen Passphrase. Ältere
Backups bleiben mit der alten verschlüsselt und lassen sich nur mit ihr
einspielen. Bewahre den alten Eintrag in Deinem Passwort-Manager deshalb
so lange auf, wie Du an ältere Backups herankommen willst. Nach dem
Einspielen eines älteren Backups gilt dessen Passphrase wieder für die
Anmeldung.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
