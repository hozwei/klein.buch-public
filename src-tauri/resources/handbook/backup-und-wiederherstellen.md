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
zusätzlich einmal je Kalendertag zu einer festen Uhrzeit (Standard 02:30
Uhr, einstellbar unter "Backups" in der Karte "Backup jetzt erstellen").
Du brauchst Dich im Alltag nicht darum zu kümmern. Wenn doch einmal ein
älterer Stand zurück muss, gibt es die Wiederherstellung (auf Englisch
"Restore").

Das Backup nach einer Festschreibung entsteht wenige Sekunden später im
Hintergrund. Du musst nicht darauf warten: Die Rechnung oder Ausgabe ist
sofort gespeichert, und Klein.Buch merkt sich, dass noch ein Backup
aussteht. Schreibst Du mehrere Belege kurz hintereinander fest, deckt ein
Backup alle ab. Sperrst Du Klein.Buch vorher oder startet der Server neu,
holt Klein.Buch das Backup kurz nach der nächsten Anmeldung nach.

Das tägliche Backup läuft nur, solange Klein.Buch entsperrt ist. Nach
einem Neustart des Servers ist Klein.Buch gesperrt, bis Du Dich wieder
anmeldest. Kurz nach der Anmeldung holt es das fällige Backup nach, und
zwar genau eines, auch wenn mehrere Tage dazwischen lagen. War Klein.Buch
länger als zwei Tage gesperrt, sagt es Dir das unter "Hinweise".

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
oder ein Backup, das schon beim Zusammenstellen scheiterte, stehen im
"Backup-Protokoll". Wie Du es liest, erklärt das Kapitel
"[Backup prüfen](/help/backup-pruefen)".

Mit "Herunterladen" neben einem Backup holst Du die Datei auf Dein Gerät,
zum Beispiel für einen Umzug auf einen neuen Server oder als zusätzliche
Kopie. Das geht für Backups im lokalen Ordner und auf Ordner-Zielen, nicht
für Kopien auf einem SFTP-Server (die holst Du dort). Klein.Buch fragt
vorher noch einmal nach Daten-Passwort und App-Code. Der Download-Link
gilt einmal und höchstens zehn Minuten; bricht der Download ab, forderst
Du ihn einfach neu an. Die Datei bleibt
verschlüsselt; ohne die Passphrase dieses Backups kann niemand sie lesen.
Bewahre sie trotzdem sorgfältig auf.

Seit Version 2026.11 schreibt Klein.Buch Backups im Format 3. Der lesbare
Kopf der Datei (Version, Zeitpunkt) ist darin an die Verschlüsselung
gebunden; eine nachträglich veränderte Datei erkennt Klein.Buch beim
Einspielen. Ältere Backups lassen sich weiter einspielen, ein Backup im
Format 3 aber nicht mit einer älteren Klein.Buch-Version.

## Aufbewahrung

Klein.Buch hebt nicht jedes Backup für immer auf, sondern sortiert ältere
automatisch aus. Es denkt dabei in Kalendertagen, Monaten und Jahren
(deutsche Zeit): Von jedem Tag, Monat und Jahr bleibt das jeweils
jüngste Backup erhalten.

Auf jedem externen Ordner-Ziel behält Klein.Buch je ein Backup für die
letzten 30 Tage mit Backup, für die letzten 12 Monate und für die letzten
7 Jahre. Im lokalen Backup-Ordner sind es 7 Tage, 3 Monate und 1 Jahr.
Die kurze lokale Reihe gilt aber nur, solange mindestens ein externes
Ziel beim letzten Mal erfolgreich war. Gibt es kein funktionierendes
externes Ziel, ist der lokale Ordner Deine einzige Kopie und behält
deshalb die lange Reihe.

Schreibst Du an einem Tag zehn Rechnungen fest, bleibt von diesem Tag nur
das letzte Backup; es enthält alles. Die Vortage verdrängt das nicht.
Gezählt werden Tage, an denen ein Backup entstand: Nach einer langen
Pause bleiben die letzten Stände also erhalten.

Nie automatisch gelöscht werden manuelle Backups ("Jetzt sichern") und
die Sicherheits-Backups vor einer Wiederherstellung. Dasselbe gilt für
die Backups, die Klein.Buch vor einer Datenbank-Umstellung nach einem
Update und nach einem Wechsel des Daten-Passworts anlegt, und für Backups,
die es nach einer Wiederherstellung im lokalen Ordner vorfindet (siehe
unten). Sie erscheinen im Verlauf als "Manuell". Diese Backups sammeln
sich mit der Zeit an. Plane den Platz dafür ein (siehe "[Speicher und
Ressourcen](/help/speicherbedarf)").

Auf SFTP-Servern löscht Klein.Buch nichts — weder Dateien noch die
Einträge im Verlauf. So kann auch eine kompromittierte Instanz Deine
externen Kopien nicht vernichten. Alte Backups auf dem SFTP-Server
räumst Du oder der Betreiber des Servers selbst auf (siehe
"[Off-Site-Backup einrichten](/help/backup-ziel-einrichten)").

Während die automatischen Abläufe angehalten sind (nach einer
Wiederherstellung oder wenn die Server-Uhr springt, siehe unten), sortiert
Klein.Buch nichts aus.

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
die auf einem externen Ziel liegen, lassen sich nicht direkt auswählen:
Hol die Datei zuerst auf Dein Gerät — bei einem Ordner-Ziel mit
"Herunterladen" in der Liste, bei einem SFTP-Ziel (in der Liste "auf dem
SFTP-Server") direkt vom Server — und lade sie dann hoch.

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
   Backup beim Neustart ein. Bis dahin nimmt er keine Änderungen an. Warte,
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

## Nach der Wiederherstellung

Nach dem Einspielen hält Klein.Buch an, bis Du ausdrücklich freigibst.
Oben erscheint dann der Hinweis "Automatische Abo-Läufe angehalten":

- Es entstehen keine Abo-Rechnungen, keine Abo-Kosten und keine
  automatischen Abschreibungen, und nichts wird automatisch versendet.
  So verschickt ein eingespielter älterer Stand keine Rechnungen, die Du
  längst gestellt hast.
- Backups laufen weiter, gehen aber nur in den lokalen Ordner. Auf die
  externen Ziele schreibt Klein.Buch erst nach der Freigabe. Sonst lägen
  dort neue Dateien mit einem alten Datenstand neben Deinen echten
  Backups — wer später "das neueste Backup" nimmt, verlöre alles danach.
- Die Rotation ruht (kein Backup wird aussortiert).

Prüfe vor der Freigabe Rechnungen, Kosten, Nummernkreise sowie Deine
Abo-Vorlagen und Abo-Kosten mit ihren nächsten Terminen. Freigeben
kannst Du mit "Prüfen und freigeben" im Hinweis oben; Klein.Buch fragt,
ob dies die produktive Instanz ist, und verlangt eine Bestätigung mit
Daten-Passwort und Authenticator-Code. Eine Probe-Instanz (zum Beispiel
zum Testen eines Backups) gibst Du nie frei.

**Nummernkreise:** Eine Rechnungsnummer darf nur einmal vergeben werden
(§14 Abs. 4 Nr. 4 UStG). Spielst Du ein älteres Backup in dieselbe
Instanz ein, übernimmt Klein.Buch deshalb die Zählerstände von vor der
Wiederherstellung, wo sie höher waren: Die nächste Rechnung bekommt die
nächste noch nie vergebene Nummer. Die Belege dazwischen fehlen in der
eingespielten Datenbank; die Lücke steht im Audit-Log, und der Hinweis
"Nummernkreise nach der Wiederherstellung" nennt die angehobenen Kreise.
Bei einer Wiederherstellung in eine neue, leere Instanz (der alte Server
ist weg) kennt Klein.Buch die späteren Nummern nicht. Dann hebst Du die
betroffenen Kreise unter "Einstellungen" → "Backups" → "Nummernkreise"
selbst auf die zuletzt vergebene Nummer an (siehe "[Wiederherstellung
nach einem Totalausfall](/help/disaster-recovery)"). Dasselbe gilt, wenn
die neue Instanz vorher schon Testdaten hatte oder das Backup von einem
anderen Server stammt: Stehen im Backup höhere Zähler als in der
ersetzten Datenbank oder tragen Belege mit derselben Nummer andere
Inhalte, sagt der Hinweis das und bittet ebenfalls ums Anheben. Nur wenn
die Stände genau übereinstimmen, meldet er "es fehlen keine Nummern".

**Entwürfe mit bereits vergebener Nummer:** Ein Rechnungs-Entwurf bekommt
seine Nummer schon beim Anlegen. Liegt im eingespielten Backup ein Entwurf,
dessen Nummer Du danach festgeschrieben und verschickt hast, ist er ein
älterer Stand dieser Rechnung. Klein.Buch merkt ihn vor und nennt ihn im
Hinweis "Nummernkreise nach der Wiederherstellung". Festschreiben lässt er
sich erst, wenn Du ihn an die verschickte Rechnung angepasst und das unter
"Einstellungen" → "Backups" → "Nummernkreise" mit "Abgleich bestätigen …"
bestätigt hast (mit Begründung, Daten-Passwort und Authenticator-Code). So
steht die verschickte Rechnung unter ihrer Nummer genau einmal in Deinen
Büchern. Dasselbe gilt für Storno-Entwürfe und Angebote.

**Abo-Vorlagen und Abo-Kosten:** Hat eine Vorlage nach dem Stand des Backups
schon eine Rechnung erzeugt (oder ein Abo schon Kosten gebucht), rückt
Klein.Buch ihren nächsten Termin auf den Stand vor der Wiederherstellung vor.
So stellt es denselben Zeitraum nach der Freigabe nicht noch einmal in
Rechnung. Der Hinweis "Abo-Termine nach der Wiederherstellung" nennt die
vorgerückten Reihen; die Rechnungen dieser Termine fehlen in der
eingespielten Datenbank (Lücke im Nummernkreis). Bei Abo-Kosten ist mehr
zu tun: Die Kosten, die das Abo nach dem Stand des Backups gebucht hatte,
sind mit der ersetzten Datenbank verworfen und fehlen jetzt in den Kosten
und in der EÜR. Der Hinweis nennt diese Abos unter "Kosten-Abos" mit den
Terminen; trag die Kosten anhand des Kontoauszugs unter "Kosten" von Hand
nach. Das Abo selbst bucht erst ab dem nächsten Termin weiter.

Kennt Klein.Buch den Stand einer Reihe nicht (etwa nach einer
Wiederherstellung in eine neue Instanz), entsteht für sie nichts
automatisch: In der Liste steht dann "Rückstand …". "Nachholen" legt die
offenen Rechnungen nur als Entwurf an, "Ab heute fortsetzen" lässt die
offenen Termine aus, auch einen heute fälligen. Ist die Reihe pausiert,
fällt dieselbe Wahl beim Fortsetzen. Abgelaufene Vorlagen und Abo-Kosten
ohne automatische Buchung betrifft das nicht; für sie entsteht ohnehin
nichts automatisch.
Bestätigungen zum Nachholen und begonnene Abo-Rechnungen aus dem Backup
gelten nach der Wiederherstellung nicht mehr.

**Backup-Verlauf:** Backups, die nach dem Stand des eingespielten Backups
im lokalen Ordner entstanden sind, nimmt Klein.Buch wieder in den Verlauf
auf (als "Manuell", sie werden nie automatisch gelöscht). So kannst Du
auch auf einen neueren Stand zurück, falls Du versehentlich ein zu altes
Backup gewählt hast.

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

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.11*
