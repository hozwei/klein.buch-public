---
slug: disaster-recovery
title: "Notfall: Instanz neu aufsetzen und wiederherstellen"
category: betrieb
order: 60
keywords: [notfall, disaster recovery, wiederherstellen, restore, neuer server, backup, kbk, passphrase]
---

# Notfall: Instanz neu aufsetzen und wiederherstellen

Dieses Kapitel ist für den Ernstfall: Der Server ist kaputt, gestohlen
oder abgebrannt, oder der Daten-Ordner lässt sich nicht mehr benutzen.
Das Ziel ist eine neue Klein.Buch-Instanz mit Deinen Daten aus dem
neuesten Backup. Eine Instanz ist eine eigenständige
Klein.Buch-Installation mit eigenen Daten. Dieses Vorgehen nennt man auch
Disaster Recovery (Wiederherstellung nach einem Totalausfall).

## Was Du brauchst

1. Eine aktuelle Backup-Datei (Endung `.kbk`) von einem Deiner externen
   Ziele, also vom NAS, aus dem Cloud-Speicher oder vom SFTP-Server. Ist
   nur der Server kaputt, der Daten-Ordner aber noch lesbar, findest Du
   die Backups auch dort im Unterordner `backups`. Eine früher mit
   "Herunterladen" geholte Datei taugt ebenso.
2. Die Passphrase, die galt, als dieses Backup erstellt wurde. Sie ist
   der einzige Schlüssel zu Deinem Backup. Ohne sie ist die Datei
   wertlos, und niemand kann sie öffnen, auch nicht der Hersteller von
   Klein.Buch.
3. Deine eigene Kopie des Vorlagen-Ordners `/inputs`, falls Du dort
   Rechnungs-Layouts oder andere Vorlagen geändert hast. Klein.Buch
   sichert diesen Ordner nicht mit. Das ist Aufgabe des Betreibers.
4. Eine Authenticator-App auf Deinem Telefon. Bei der Einrichtung
   entsteht dort ein neuer Eintrag.
5. Genug Arbeitsspeicher und Platz auf dem neuen Server (siehe unten).

## Das richtige Backup wählen

Nimm das neueste Backup. Klein.Buch legt nach jeder Festschreibung ein
Backup an und kopiert es auf alle externen Ziele. Das neueste externe
Backup enthält deshalb normalerweise auch Deine letzte Rechnung,
vorausgesetzt, die Kopie hat geklappt. Der Dateiname enthält Datum und
Uhrzeit der Sicherung, zum Beispiel
`klein-buch-20261024-143005123-1a2b3c4d.kbk`.

Vergleiche diesen Zeitpunkt mit Deiner zuletzt verschickten Rechnung.
Alles, was nach dem Backup festgeschrieben wurde, fehlt nach der
Wiederherstellung. Die neue Instanz kennt diese Belege nicht und damit
auch nicht ihre Nummern. Damit keine Nummer doppelt vergeben wird
(§14 Abs. 4 Nr. 4 UStG), hebst Du die betroffenen Nummernkreise vor der
ersten neuen Rechnung an (Schritt 4). Eine Lücke ist zulässig und
dokumentiert, eine doppelte Nummer nicht. Abo-Rechnungen, Abo-Kosten und
der Versand bleiben nach der Wiederherstellung angehalten, bis Du sie
freigibst; es geht also nichts automatisch hinaus. Klein.Buch ist ein
Werkzeug, kein Steuerberater. Bei Lücken oder doppelten Rechnungen sprich
mit Deiner Steuerberatung, bevor Du etwas stornierst.

## Schritt 1: Neue Instanz anlegen

Der Betreiber installiert Klein.Buch neu, wie im Kapitel "[Installation mit Docker Compose](/help/installation-docker)" beschrieben. Dabei gilt:

1. Der Ordner für `/data` muss leer sein. Liegen dort noch
   Archiv-Dateien ohne Datenbank, verweigert Klein.Buch den Start. So
   kann niemand versehentlich eine neue Instanz über einem alten Archiv
   einrichten.
2. Die Version des Images muss mindestens so neu sein wie die, aus der
   das Backup stammt. Eine ältere Version lehnt neuere Backups ab. Eine
   neuere Version stellt ältere Backups selbst um.
3. Die Backup-Ziele bindet der Betreiber unter denselben Namen ein wie
   vorher, zum Beispiel wieder als `/backup-targets/nas`. Die Liste der
   Ziele kommt mit dem Backup zurück, und Klein.Buch erkennt die Ordner
   an diesen Namen.
4. Seine Kopie von `/inputs` legt der Betreiber vor dem ersten Start in
   den neuen Vorlagen-Ordner. Klein.Buch ergänzt beim Containerstart fehlende
   Dateien aus dem Image und überschreibt nie eine Datei, die Du geändert
   hast. Bringt das Image eine neuere Fassung einer solchen Datei mit,
   legt Klein.Buch sie mit der Endung `.neu` daneben und meldet das unter
   "Hinweise".
5. Der Container muss nach dem Beenden von selbst neu starten
   (`restart: unless-stopped` wie im Compose-Beispiel). Die
   Wiederherstellung braucht das.
6. Arbeitsspeicher und Platz reichen für die Wiederherstellung (siehe
   "Was eine Wiederherstellung braucht").

## Schritt 2: Die neue Instanz einrichten

Richte die neue Instanz ein wie beim ersten Mal (siehe Kapitel
"[Passphrase einrichten](/help/passphrase-einrichten)"): Setup-Token aus
dem Container-Log, eine Passphrase, QR-Code in der Authenticator-App,
Einmal-Codes notieren.

Die Passphrase, die Du hier festlegst, gilt nur bis zur
Wiederherstellung. Danach gilt die Passphrase aus dem Backup. Den neuen
Eintrag in der Authenticator-App dagegen behältst Du. Die Verbindung zur
App ist nicht Teil des Backups, deshalb bleibt sie nach der
Wiederherstellung die der neuen Instanz. Der Eintrag der alten Instanz
funktioniert nicht mehr. Die neuen Einmal-Codes gelten weiter.

## Schritt 3: Das Backup einspielen

1. Kopiere die Backup-Datei vom externen Ziel vollständig auf Dein Gerät.
2. Öffne in der neuen Instanz "Einstellungen" und "Backups" und wähle in
   der Karte "Wiederherstellen" im Feld "Backup-Datei (.kbk)" die `.kbk`-Datei.
3. Gib im Feld "Backup-Passwort" die Passphrase ein, die galt, als das
   Backup erstellt wurde.
4. Klick auf "Wiederherstellen" und bestätige die Rückfrage "Backup
   wiederherstellen?" mit "Wiederherstellen". Fragt Klein.Buch danach im
   Fenster "Aktion bestätigen: Backup wiederherstellen" nach
   Daten-Passwort und App-Code, sind das die Passphrase und der Code der
   neuen Instanz aus Schritt 2.

Klein.Buch lädt die Datei hoch und prüft sie: Passphrase, Vollständigkeit,
Version und ob die Datenbank im Backup zu diesem Server passt. Erst dann
legt es ein Sicherheits-Backup des aktuellen Stands an. Das ist vor
jeder Wiederherstellung Pflicht, auch wenn die neue Instanz noch leer
ist. Danach meldet es "Wiederherstellung vorbereitet", der Server startet
neu und spielt das Backup beim Neustart ein. Das Sicherheits-Backup der
leeren Instanz steht später im Backup-Verlauf ("Vor einer
Wiederherstellung"). Du kannst es ignorieren.

## Schritt 4: Anmelden und prüfen

Melde Dich mit der Passphrase aus dem Backup und dem Code aus dem neuen
Eintrag Deiner Authenticator-App an. Stammt das Backup aus einer älteren
Version, legt Klein.Buch jetzt erst ein Backup an und stellt dann die
Datenbank auf das aktuelle Format um.

Danach prüfst Du:

1. Unter "Rechnungen", "Kosten" und den anderen Belegen: Welche Nummer
   wurde nach dem Stand des Backups zuletzt vergeben (Mail-Ausgang,
   Korrespondenz, Kontoauszug)? Fehlen Belege, hebst Du unter
   "Einstellungen" → "Backups" → "Nummernkreise" den betroffenen Kreis
   an: Klick in seiner Zeile auf "Anheben …", trag im Feld "Zuletzt
   vergebene Nummer (laufende Nummer)" die zuletzt vergebene laufende
   Nummer und im Feld "Begründung (Audit-Log)" mindestens 10 Zeichen ein
   und klick auf "Nummernkreis anheben". Die Rückfrage nennt die nächste
   Nummer; Du bestätigst mit "Anheben" und, falls Klein.Buch danach
   fragt, mit Daten-Passwort und App-Code. Klein.Buch lässt nur ein
   Anheben zu, nie ein Absenken. Der Hinweis "Nummernkreise nach der
   Wiederherstellung" unter "Hinweise" erinnert daran.
2. Unter "Einstellungen" und "Backups": Sind die externen Ziele wieder
   da? Bis zur Freigabe (Punkt 5) gehen Backups nur in den lokalen
   Ordner; nach "Jetzt sichern" stehen die Ziele deshalb als
   "Ausgesetzt". Unter "Rechnungen" → "Abo-Rechnungen" und "Kosten" →
   "Kosten-Abos": Reihen mit einem offenen Termin bis heute
   zeigen "Rückstand …"
   (pausierte Reihen fragen beim Fortsetzen). Prüfe im Mail-Ausgang bzw.
   auf dem Kontoauszug, ob der Termin auf dem alten Server schon
   abgerechnet wurde. "Ab heute fortsetzen" lässt die offenen Termine aus,
   auch einen heute fälligen; "Nachholen" legt Rechnungen nur als Entwurf
   an.
3. Unter "Hinweise": Meldet Klein.Buch "PDF-Vorlagen nach dem Restore
   fehlen", fehlt in `/inputs` eine Vorlage, die Deine Daten benutzen.
   Dann muss der Betreiber seine Kopie von `/inputs` zurücklegen.
4. Unter "Einstellungen" und "Protokoll & Datensicherheit": Starte in der
   Karte "Archiv-Integrität" mit "Jetzt prüfen" eine Prüfung des
   Archivs.
5. Erst wenn alles stimmt: Oben im Hinweis "Automatische Abo-Läufe
   angehalten." auf "Prüfen und freigeben" klicken, die Rückfrage "Ist
   dies die produktive Instanz?" mit "Ja, produktiv — freigeben"
   beantworten und, falls Klein.Buch danach fragt, mit Daten-Passwort
   und App-Code bestätigen. Danach laufen Abo-Rechnungen, Abo-Kosten und
   Versand wieder (größere Rückstände nur nach Rückfrage),
   und Backups gehen wieder auf die externen Ziele. Klick danach auf
   "Jetzt sichern" und schau, ob die Kopie auf jedes Ziel klappt.

Mail-Konten und SFTP-Ziele samt Passwörtern kommen mit dem Backup zurück,
auf dem Stand des Backups.

Kommt der alte Server doch wieder in Gang, betreibe nie beide Instanzen
weiter. Zwei Instanzen mit demselben Bestand vergeben dieselben
Rechnungsnummern doppelt.

## Was eine Wiederherstellung braucht

Arbeitsspeicher: Klein.Buch hält die Backup-Datei beim Entschlüsseln
vollständig im Arbeitsspeicher und entschlüsselt sie an derselben Stelle.
Rechne mit etwa der Backup-Größe, zusätzlich zum Grundbedarf. Für ein
Backup von 1 GiB ist das rund 1 GiB. Erst nach dem Auspacken, wenn
dieser Speicher wieder frei ist, legt Klein.Buch das Sicherheits-Backup
des bisherigen Stands an; das braucht bis zum Doppelten von dessen
Größe. In einer frisch eingerichteten Instanz ist es winzig, über einen
gewachsenen Bestand gilt die Faustregel aus dem Kapitel
"Speicherbedarf". Ist die Speichergrenze des
Containers (`mem_limit` in der Compose-Datei) kleiner, bricht die
Wiederherstellung ab. Der Betreiber kann die Grenze für die
Wiederherstellung anheben und danach wieder senken.

Platz im Daten-Ordner: Die hochgeladene Datei selbst, das
Sicherheits-Backup und das Doppelte des entpackten Inhalts, weil
Klein.Buch ihn erst vorbereitet und beim Neustart an seinen Platz
kopiert. Dazu kommt eine Reserve, die immer frei bleiben muss.

Zwei Umgebungsvariablen setzen die Grenzen. `KLEIN_BUCH_MAX_RESTORE_BYTES`
ist die größte erlaubte Backup-Datei und zugleich die größte erlaubte
entpackte Menge, Standard 4 GiB. `KLEIN_BUCH_MIN_FREE_BYTES` ist die
Reserve an freiem Platz, Standard 2 GiB. Beide Werte sind reine
Byte-Zahlen, zum Beispiel `8589934592` für 8 GiB. Eine Angabe wie `8G`
versteht Klein.Buch nicht und bricht den Start ab. Mehr dazu im Kapitel
"[Speicher und Ressourcen](/help/speicherbedarf)".

## Was nicht geht

Ohne die passende Passphrase lässt sich kein Backup einspielen. Es gibt
keinen Weg darum herum.

Backups aus einer älteren Version als Klein.Buch 2026.5 nimmt der Server
nicht an.

Einen Umzug aus der früheren Desktop-Fassung gibt es nicht. Ein
Desktop-Backup mit festgeschriebenen Belegen lehnt Klein.Buch ab, weil
die Belege dort unter Pfaden der Desktop-Fassung abgelegt sind. Wer die
Desktop-Fassung produktiv genutzt hat, bewahrt den Bestand mit dieser
Fassung auf.

Klein.Buch erwartet seinen Daten-Ordner im Container immer unter
`/data`, so ist das Image eingestellt. Backups einer Instanz mit einem
anderen Daten-Ordner passen nicht.

Die Verbindung zur Authenticator-App und der Vorlagen-Ordner `/inputs`
stecken nicht im Backup. Den zweiten Faktor richtest Du in Schritt 2
neu ein. `/inputs` sichert der Betreiber selbst.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
