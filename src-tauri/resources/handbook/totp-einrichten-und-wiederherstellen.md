---
slug: totp-einrichten-und-wiederherstellen
title: Zweiter Faktor (TOTP) einrichten und wiederherstellen
category: betrieb
order: 40
keywords: [totp, zweiter faktor, authenticator, einmal-codes, totp-reset, gerät verloren, setup-token, re-auth, anmeldung, sitzung]
---

# Zweiter Faktor (TOTP) einrichten und wiederherstellen

Weil Klein.Buch aus dem Internet erreichbar ist, verlangt die Anmeldung
zwei Dinge: Dein Daten-Passwort und einen Code aus einer
Authenticator-App auf Deinem Handy. Das Verfahren heißt TOTP
(Time-based One-Time Password, zeitbasiertes Einmal-Passwort): Die App
und Klein.Buch kennen dasselbe Geheimnis und errechnen daraus alle 30
Sekunden denselben sechsstelligen Code. Wer nur Dein Passwort kennt,
kommt nicht hinein, und wer nur Dein Handy hat, auch nicht.

## Einrichten

Den zweiten Faktor richtest Du bei der ersten Einrichtung ein (Kapitel
"Installation mit Docker Compose"). Klein.Buch zeigt einen QR-Code und
darunter denselben Schlüssel zum Abtippen. Jede gängige
Authenticator-App funktioniert, zum Beispiel Aegis, 2FAS oder Google
Authenticator. Du bestätigst mit dem ersten Code aus der App.

Danach zeigt Klein.Buch zehn Einmal-Codes. Jeder davon ersetzt genau
einmal den Code aus der App, nicht das Daten-Passwort. Sie werden nur
in diesem Moment angezeigt; Klein.Buch speichert sie nur als
Prüfsumme und kann sie Dir nicht noch einmal zeigen. Bewahre sie
getrennt vom Handy auf, zum Beispiel ausgedruckt oder in einem
Passwort-Manager.

Unter "Einstellungen" und "Sicherheit & Anmeldung" siehst Du, wie viele
Einmal-Codes noch übrig sind. Mit **"Neue Einmal-Codes erzeugen"**
bekommst Du zehn neue; die alten gelten dann nicht mehr. Mit **"Zweiten
Faktor neu einrichten"** wechselst Du auf ein neues Handy oder eine
neue App, solange Du noch Zugang hast: neuen QR-Code scannen, Code
eingeben, **"Neu einrichten"**. Dabei entstehen ebenfalls zehn neue
Einmal-Codes.

## Wenn ein Code nicht passt

Der Code hängt an der Uhrzeit. Klein.Buch nimmt den aktuellen Code und
den davor und danach an, also etwa eine halbe Minute Abweichung. Geht
die Uhr Deines Handys oder Deines Servers falsch, passt kein Code.
Stelle beide auf automatische Zeit (auf dem Server über NTP, die
Zeitabfrage im Netz).

Jeder Code gilt nur einmal, auch über einen Neustart hinweg. Ein Code
zählt als verbraucht, sobald er stimmt, auch wenn danach das
Daten-Passwort falsch war. Hast Du Dich beim Passwort vertippt, warte
also auf den nächsten Code.

Nach fünf Fehlversuchen musst Du eine Minute warten, danach verdoppelt
sich die Wartezeit bis höchstens eine Stunde. Eine dauerhafte Sperre
gibt es nicht; der Zähler verfällt 24 Stunden nach dem letzten
Fehlversuch. Dein eigener Browser bekommt nach der ersten Anmeldung ein
Gerätecookie und muss deshalb nicht warten, wenn jemand anderes
Passwörter durchprobiert. Nach der Anmeldung meldet Klein.Buch, wie viele
Fehlversuche es seit Deiner letzten Anmeldung gab.

## Handy verloren, Einmal-Codes vorhanden

Klick bei der Anmeldung auf "Gerät verloren? Einmal-Code verwenden" und
gib Daten-Passwort und einen Einmal-Code ein. Richte danach sofort unter
"Einstellungen" und "Sicherheit & Anmeldung" den zweiten Faktor auf dem
neuen Gerät neu ein.

## Handy und Einmal-Codes verloren

Dann setzt Du den zweiten Faktor auf dem Server zurück. Dafür brauchst
Du Zugriff auf die Kommandozeile des Servers, auf dem der Container
läuft:

```
docker exec klein-buch klein-buch-server totp-reset
```

`klein-buch` ist der Name aus `container_name`. Wenn Du ohne festen
Namen arbeitest, geht es im Ordner der Compose-Datei auch mit
`docker compose exec klein-buch klein-buch-server totp-reset`.
Klein.Buch antwortet mit "TOTP-Geheimnis gelöscht." und beendet alle
Sitzungen. Der Befehl löscht nur das Geheimnis des zweiten Faktors.
Datenbank, Belege und Daten-Passwort bleiben unverändert.

Öffne dann Klein.Buch im Browser. Es zeigt "Zweiten Faktor neu
einrichten" und schreibt gleichzeitig einen neuen Setup-Token ins
Container-Log. Hol ihn Dir mit `docker compose logs klein-buch` (der
Block "=== Klein.Buch: Einrichtung nötig ==="). Gib Setup-Token und Dein
bisheriges Daten-Passwort ein, scanne den neuen QR-Code, bestätige mit
einem Code und notiere die neuen Einmal-Codes. War Klein.Buch gesperrt,
etwa nach einem Neustart, entsperrt dieser Schritt es zugleich.

Wer auf dem Server Docker-Befehle ausführen darf, kann den zweiten
Faktor also zurücksetzen. Ohne Daten-Passwort kommt er trotzdem nicht an
Deine Daten. Schütze den Zugang zum Server entsprechend.

Ein verlorenes Daten-Passwort lässt sich auf diesem Weg nicht ersetzen.
Ohne Daten-Passwort sind Datenbank und Backups nicht mehr lesbar, das
ist Absicht (Kapitel "Passphrase einrichten").

## Erneute Bestätigung bei heiklen Aktionen

Auch mit gültiger Anmeldung fragt Klein.Buch bei manchen Aktionen noch
einmal nach Daten-Passwort und Code (oder einem Einmal-Code). Die
Bestätigung gilt fünf Minuten. Das betrifft: ein Backup
wiederherstellen, Backup-Ziele anlegen, entfernen und testen,
Mailkonten anlegen, ändern, löschen und testen, eine Testmail senden,
den Export für den Steuerberater, den Detail-Export, den
Migrations-Export, die DSGVO-Auskunft, neue Einmal-Codes, den zweiten
Faktor neu einrichten und das Daten-Passwort ändern. So kann jemand,
der Dein offenes Browserfenster erwischt, keine Daten abziehen und
keine Zugänge ändern.

Eine Anmeldung endet nach 30 Minuten ohne Aktivität und spätestens nach
12 Stunden. "Datenbank sperren" beendet alle Sitzungen und verschließt
die Datenbank, ebenso jeder Neustart des Containers.

Das Geheimnis des zweiten Faktors liegt außerhalb der Datenbank unter
`/data/auth/` und gehört nicht zu den Backups. Nach einer
Wiederherstellung auf einem neuen Server richtest Du den zweiten Faktor
deshalb neu ein.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
