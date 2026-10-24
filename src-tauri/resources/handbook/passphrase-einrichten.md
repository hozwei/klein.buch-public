---
slug: passphrase-einrichten
title: Passphrase einrichten
category: erste-schritte
order: 20
keywords: [passphrase, daten-passwort, einrichten, setup-token, totp, authenticator, anmelden, verschlüsselung, sqlcipher]
---

# Passphrase einrichten

Klein.Buch verschlüsselt Deine Datenbank und alle Backups mit einer
Passphrase. Eine Passphrase ist ein Passwort, meistens länger und aus
mehreren Wörtern. In Klein.Buch heißt sie an den meisten Stellen
"Daten-Passwort", auf der Backup-Seite "Backup-Passwort". Gemeint ist
immer dasselbe.

Zur Anmeldung gehört außerdem ein zweiter Faktor: ein sechsstelliger
Code aus einer Authenticator-App auf Deinem Telefon. Das Verfahren heißt
TOTP (zeitbasiertes Einmal-Passwort). Die App erzeugt alle 30 Sekunden
einen neuen Code. Wer nur Deine Passphrase kennt, kommt ohne diesen Code
nicht hinein, und umgekehrt.

Beides legst Du ein einziges Mal fest, wenn Klein.Buch zum ersten Mal
startet. Die Einrichtung läuft komplett im Browser und in einem Schritt.

## Bevor Du beginnst

Notiere Dir die Passphrase außerhalb von Klein.Buch, am besten in einem
Passwort-Manager. Verlierst Du sie, sind Deine Daten und alle Backups
unwiederbringlich weg. Es gibt absichtlich keine Hintertür und keinen
Knopf, der die Daten zurückbringt. Genau das sorgt dafür, dass niemand
Deine Daten lesen kann, der den Server oder ein Backup in die Hände
bekommt.

Installiere außerdem eine Authenticator-App auf Deinem Telefon, zum
Beispiel Aegis, 2FAS oder Google Authenticator.

## Was eine gute Passphrase ist

Eine gute Passphrase besteht aus drei bis vier zusammenhängenden, aber
nicht offensichtlichen Wörtern und hat insgesamt mindestens 16 Zeichen.
Beispiel-Muster (nicht zum Übernehmen):
`<wort>-<wort>-<wort>-<zahl><sonderzeichen>`. Vermeide Geburtstage,
Namen von Familienmitgliedern, "passwort123" und alle Passwörter, die Du
schon irgendwo anders verwendest.

Klein.Buch erzwingt nur die Mindestlänge von 16 Zeichen. Alles andere
ist Empfehlung. Du gibst die Passphrase zweimal ein, damit ein
Tippfehler nicht unbemerkt bleibt.

## Der Setup-Token

Damit nicht irgendjemand aus dem Internet Deine frisch gestartete
Instanz einrichtet, verlangt Klein.Buch beim ersten Mal einen
Setup-Token. Das ist eine zufällige Zeichenfolge, die Klein.Buch beim
Start in das Container-Log schreibt (die Textausgabe des Containers, die
der Betreiber auf dem Server lesen kann). Dort steht ein Block mit der
Überschrift "Klein.Buch: Einrichtung nötig" und darunter die Zeile
"Setup-Token:". Der Betreiber findet ihn zum Beispiel mit dem Befehl
`docker compose logs klein-buch`. Der Token gilt bis zum nächsten
Neustart des Containers. Nach einem Neustart steht ein neuer im Log.

Einrichtung und Anmeldung klappen nur über eine verschlüsselte
Verbindung, also über eine Adresse mit `https://`, die der Reverse-Proxy
des Betreibers bereitstellt. Ein Reverse-Proxy ist das Programm, das die
verschlüsselte Verbindung zum Browser herstellt und die Anfragen an
Klein.Buch weiterreicht. Wie das eingerichtet wird, steht in den
Kapiteln "[Installation mit Docker Compose](/help/installation-docker)" und
"[Reverse-Proxy und TLS](/help/reverse-proxy-und-tls)".

## Schritte

1. Öffne die Adresse Deiner Klein.Buch-Instanz im Browser. Es erscheint
   das Fenster "Klein.Buch einrichten".
2. Trag den Setup-Token aus dem Container-Log in das Feld "Setup-Token
   (aus dem Container-Log)" ein.
3. Tipp Deine Passphrase in das Feld "Passwort" und wiederhole sie im
   Feld "Passwort wiederholen". Klick auf "Weiter zum QR-Code".
4. Im Fenster "Authenticator-App verbinden" siehst Du einen QR-Code.
   Scanne ihn mit Deiner Authenticator-App. Klappt das Scannen nicht,
   tippst Du den "Schlüssel zum Abtippen" in die App ein.
5. Gib den sechsstelligen Code, den die App jetzt anzeigt, in das Feld
   "Code aus der App" ein und klick auf "Einrichten & erstes Backup
   erstellen".
6. Klein.Buch zeigt Dir zehn Einmal-Codes. Mit jedem davon kommst Du
   einmal hinein, falls Dein Telefon mit der Authenticator-App verloren
   geht. Ein Einmal-Code ersetzt nur den App-Code, nicht die Passphrase.
   Die Codes werden nur dieses eine Mal angezeigt. Schreib sie ab oder
   leg sie im Passwort-Manager ab, setz das Häkchen bei "Ich habe die
   Codes sicher notiert." und klick auf "Weiter".

Klein.Buch hat jetzt die verschlüsselte Datenbank angelegt, die
Verschlüsselung eingerichtet und ein erstes Backup geschrieben. Du bist
angemeldet und landest auf der Startseite.

Als Nächstes trägst Du Deine Firmendaten ein. Geh dafür auf
"Einstellungen" und "Meine Firmendaten" (siehe Kapitel "[Verkäuferprofil
anlegen](/help/verkaeuferprofil-anlegen)"). Danach richtest Du ein
externes Backup-Ziel ein (siehe Kapitel "[Off-Site-Backup
einrichten](/help/backup-ziel-einrichten)").

## Anmelden

Später meldest Du Dich im Fenster "Anmelden" mit Deinem Daten-Passwort
und dem aktuellen Code aus der Authenticator-App an. Hast Du das Telefon
nicht zur Hand, klickst Du auf "Gerät verloren? Einmal-Code verwenden"
und gibst einen Deiner Einmal-Codes ein.

Eine Anmeldung gilt für diesen Browser. Nach 30 Minuten ohne Aktivität
und spätestens nach 12 Stunden musst Du Dich neu anmelden. Im Hauptmenü
beendet "Abmelden" nur die Anmeldung in diesem Browser. "Datenbank
sperren" schließt die Datenbank und meldet alle Geräte ab.

## Nach einem Neustart ist Klein.Buch gesperrt

Startet der Server oder der Container neu, zum Beispiel nach einem
Update oder Stromausfall, ist Klein.Buch gesperrt. Die Datenbank bleibt
verschlüsselt, bis sich jemand mit Passphrase und Code anmeldet. Das ist
Absicht: Klein.Buch speichert Deine Passphrase nirgends, also kann es
sich auch nicht selbst entsperren.

Solange Klein.Buch gesperrt ist, laufen keine geplanten Aufgaben: keine
Abo-Rechnungen, kein tägliches Backup, keine Archiv-Prüfung. Nach Deiner
Anmeldung holt Klein.Buch Fälliges nach, frühestens eine Minute später:
genau ein Tages-Backup, bis zu drei offene Termine je Abo; größere
Rückstände erst nach Deiner Rückfrage (Kapitel "Abo-Rechnungen"). Melde
Dich nach einem Neustart deshalb bald wieder an.

## Was technisch im Hintergrund passiert

Klein.Buch verschlüsselt die Datenbank-Datei mit SQLCipher (einer
Verschlüsselungs-Schicht für die Datenbank) und leitet aus Deiner
Passphrase über ein standardisiertes Verfahren (PBKDF2-HMAC-SHA512) den
Datenbank-Schlüssel ab. Backups bekommen zusätzlich eine eigene
verschlüsselte Hülle mit einem zweiten Standard-Verfahren (Argon2id).
Die Passphrase selbst liegt nirgends auf dem Server, weder im Klartext
noch verschlüsselt, und sie erscheint in keinem Log. Solange Klein.Buch
entsperrt ist, hält es den Schlüssel nur im Arbeitsspeicher.

Das Geheimnis für den App-Code liegt getrennt von der Datenbank in einer
eigenen Datei auf dem Server. Allein nützt es niemandem, denn ohne
Passphrase bleibt die Datenbank verschlossen.

Dasselbe Geheimnis, Deine Passphrase, schützt also Datenbank und
Backups. Eine kopierte Datenbank oder ein kopiertes Backup ohne
Passphrase ist wertlos. Auch der Hersteller von Klein.Buch kann das
nicht entschlüsseln.

## Passphrase ändern

Unter "Einstellungen" und "Sicherheit & Anmeldung" gibt es die Karte
"Daten-Passwort ändern". Du gibst das neue Passwort zweimal ein
(mindestens 16 Zeichen), klickst auf "Passwort ändern" und bestätigst die
Rückfrage. Eventuell fragt Klein.Buch vorher im Fenster "Aktion
bestätigen" noch einmal nach dem bisherigen Daten-Passwort und dem
App-Code.

Klein.Buch verschlüsselt die Datenbank dann mit dem neuen Passwort neu,
meldet andere angemeldete Geräte ab und legt ein Backup mit dem neuen
Passwort an. Ältere Backups bleiben mit dem bisherigen Passwort
verschlüsselt und lassen sich nur mit ihm einspielen. Bewahre das alte
Passwort deshalb im Passwort-Manager auf.

## Wenn Du das Telefon mit der Authenticator-App verlierst

Dann meldest Du Dich mit einem Einmal-Code an und richtest unter
"Einstellungen" und "Sicherheit & Anmeldung" mit "Zweiten Faktor neu
einrichten" ein neues Telefon ein. Hast Du auch keine Einmal-Codes mehr,
kann der Betreiber den zweiten Faktor auf dem Server zurücksetzen.
Danach richtest Du ihn mit einem neuen Setup-Token und Deinem
bisherigen Daten-Passwort neu ein. Wie das geht, steht im Kapitel
"[Zweiter Faktor (TOTP) einrichten und wiederherstellen](/help/totp-einrichten-und-wiederherstellen)".

## Wenn Du die Passphrase vergessen hast

Es gibt keinen Weg zurück. Wirklich keinen. Weder der Betreiber noch der
Hersteller kann die Datenbank oder die Backups ohne Passphrase öffnen.
Hast Du sie nirgends notiert, sind Deine Buchhaltungs-Daten endgültig
verloren. Möglich ist dann nur ein Neuanfang mit einer neuen, leeren
Instanz (siehe Kapitel "[Notfall: Instanz neu aufsetzen und
wiederherstellen](/help/disaster-recovery)"). Deine alten Belege sind
damit nicht zurück.

Deshalb dieser Satz noch einmal: Passphrase in den Passwort-Manager.
Sofort.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
