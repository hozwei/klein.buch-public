---
slug: fehlersuche-im-container
title: Fehlersuche im Container
category: betrieb
order: 80
keywords: [fehlersuche, docker logs, log, healthcheck, startfehler, fail2ban, fehlversuche, gesperrt, neustart, instanz-marker]
---

# Fehlersuche im Container

Fast jede Störung im Betrieb erklärt das Container-Log. Dort schreibt
Klein.Buch beim Containerstart jede Prüfung, die scheitert, und im Betrieb alle
Fehler, die es der Oberfläche aus Sicherheitsgründen nur allgemein
meldet. Dieses Kapitel zeigt, wo Du nachsiehst und was die häufigsten
Meldungen bedeuten.

## Log und Zustand ansehen

Im Ordner der Compose-Datei zeigt `docker compose logs klein-buch` das
Log, mit `-f` fortlaufend und mit `--since 1h` nur die letzte Stunde.
Ohne Compose geht es mit `docker logs klein-buch`. Jede Zeile beginnt
mit Datum und Uhrzeit in UTC (koordinierte Weltzeit, im Winter eine
Stunde und im Sommer zwei Stunden hinter der deutschen Uhrzeit), dann
folgen die Stufe (`INFO`, `WARN`, `ERROR`) und der Bereich. Mehr
Einzelheiten bekommst Du mit der Umgebungsvariable `RUST_LOG=debug`;
Dein Daten-Passwort steht auch dann nicht im Log. E-Mail-Adressen von
Kunden stehen nur bei `debug` im Log.

Jeder Aufruf der Oberfläche hinterlässt eine Zeile im Bereich
`klein_buch_lib::http`, etwa:

```
2026-10-25T09:15:02.123456Z  INFO klein_buch_lib::http: POST /api/cmd/invoices_lock 500 41 ms ip=203.0.113.7 req=0d4e…-…-… sess=1a2b3c4d
```

`ip` ist die Adresse des Browsers, `sess` eine Kurzkennung der
Sitzung. Dieselbe Kurzkennung siehst Du unter Einstellungen →
Sicherheit bei "Angemeldete Sitzungen". `req` ist die Kennung des
Aufrufs. Meldet die Oberfläche "Interner Fehler (Ref …)", ist die Ref
genau diese Kennung. Mit
`docker compose logs klein-buch | grep <Ref>` findest Du die
Zugriffszeile und den eigentlichen Fehler. Die IP-Adressen bleiben nur
so lange im Log, wie die Log-Rotation der Compose-Datei es zulässt
(`max-size` und `max-file`, im Beispiel 5 Dateien zu 10 MB).

Ob Klein.Buch läuft, prüft Docker alle 30 Sekunden selbst. Das Ergebnis
zeigt `docker ps` in der Spalte "STATUS" (`healthy` oder `unhealthy`).
Genauer wird es mit:

```
docker exec klein-buch klein-buch-server healthcheck
```

Die Ausgabe nennt den Zustand (`state`): `setup` heißt noch nicht
eingerichtet, `locked` gesperrt, `unlocked` entsperrt. Im entsperrten
Zustand steht dort außerdem der Stand der externen Backups (`offsite`):
`missing` (kein Ziel eingerichtet), `failing` (ein Ziel ist zuletzt
gescheitert) oder `ok`. Die laufende Version zeigt
`docker exec klein-buch klein-buch-server version`.

## Gesperrt nach jedem Neustart

Nach jedem Start des Containers ist Klein.Buch gesperrt, bis Du Dich im
Browser anmeldest. Das gilt nach einem Update, nach einem Neustart des
Servers und nach "Datenbank sperren". Der Grund: Dein Daten-Passwort ist
zugleich der Schlüssel der Datenbank. Ein automatisches Entsperren
müsste diesen Schlüssel auf dem Server ablegen, und wer die Festplatte
stiehlt, hätte dann Deine Daten im Klartext.

Solange Klein.Buch gesperrt ist, läuft nichts Geplantes: kein
Tages-Backup, keine Abo-Rechnungen, keine Prüfung des Archivs, keine
Versionsprüfung. Das Log sagt das beim Containerstart und danach alle sechs
Stunden mit der Zeile "Instanz gesperrt — Backups, Abo-Läufe und
Prüfungen ruhen bis zur Anmeldung im Browser". Eine bis fünf Minuten nach
der Anmeldung holt Klein.Buch Fälliges nach, gestaffelt: zuerst Abo-Läufe und
genau ein Backup, die Archiv-Prüfung frühestens fünf Minuten später. War
die Instanz länger als zwei Tage gesperrt, steht unter "Hinweise", wie
lange. Der Healthcheck zählt "gesperrt" als gesund, damit Docker den
Container nicht endlos neu startet. Willst Du gewarnt werden, wenn
Klein.Buch nach einem nächtlichen Neustart gesperrt bleibt, lass Deine
Überwachung die Ausgabe des Healthcheck-Befehls auf `locked` prüfen.

## Typische Fehler beim Containerstart

Scheitert eine Prüfung, steht im Log eine Zeile der Stufe `ERROR` mit
"Start abgebrochen:" und dem Grund, und Docker startet den Container
immer wieder neu. Die häufigsten Gründe:

- "KLEIN_BUCH_PUBLIC_URL fehlt" oder "KLEIN_BUCH_PUBLIC_URL=… ungültig":
  Die öffentliche Adresse fehlt oder hat einen Pfad, einen Abfrage-Teil
  oder Benutzerangaben (Kapitel "Reverse-Proxy und TLS").
- "KLEIN_BUCH_TRUSTED_PROXIES: „…“ ist kein CIDR-Bereich": ein
  Tippfehler in der Liste der vertrauten Proxys.
- "Daten-Verzeichnis /data fehlt" oder "… ist kein eingebundenes
  Volume": Das Volume ist in der Compose-Datei nicht oder falsch
  eingetragen. Dasselbe gibt es für "Inputs-Verzeichnis /inputs".
- "… ist nicht beschreibbar": Der Ordner gehört nicht dem Benutzer aus
  der Zeile `user`. Korrigiere den Besitzer mit `chown` (Kapitel
  "Volumes und Rechte").
- "Selbsttest im Archiv … gescheitert": Der Schreibschutz der Belege
  greift nicht, weil der Container als `root` läuft oder das Dateisystem
  das Recht `0400` nicht umsetzt, etwa eine Netzwerkfreigabe.
- "Auf /data läuft bereits ein Klein.Buch-Prozess": Zwei Container
  benutzen denselben Datenordner. Pro Datenordner darf nur einer laufen.
- "Datenbank … ohne Instanz-Marker": In `/data` liegt eine Datenbank,
  aber der Marker `.klein-buch-instance` fehlt. Meist ist ein fremder
  Ordner eingebunden oder eine Einrichtung wurde abgebrochen.
- "Instanz-Marker … ohne Datenbank": Der Marker ist da, die Datenbank
  fehlt. Der Ordner ist unvollständig, zum Beispiel nach einer halben
  Kopie. Prüfe den Ordner oder spiel ein Backup ein.
- "Archiv … enthält Dateien, aber es gibt weder Datenbank noch
  Instanz-Marker": Klein.Buch richtet sich nie über einem vorhandenen
  Archiv neu ein, damit keine Belege verloren gehen.
- "… ist unverschlüsselt": Im Datenordner liegt eine unverschlüsselte
  Datenbank. Der Server öffnet nur verschlüsselte.
- "Port … nicht verfügbar": Die Adresse aus `KLEIN_BUCH_LISTEN` lässt
  sich im Container nicht belegen.

Beim Marker und beim Archiv gilt: Lösch nichts auf Verdacht. Klein.Buch
bricht genau dann ab, wenn es befürchtet, dass Daten verloren gehen
könnten. Prüfe erst, ob der richtige Ordner eingebunden ist.

## Typische Fehler im Betrieb

"Unbekannter Host." und "Anmeldung nur über HTTPS (Reverse-Proxy) oder
lokal möglich." betreffen fast immer den Reverse-Proxy; das Kapitel
"Reverse-Proxy und TLS" erklärt beide. "Anfrage von fremder Herkunft
abgelehnt." erscheint, wenn Du Klein.Buch unter einer anderen Adresse
aufrufst als in `KLEIN_BUCH_PUBLIC_URL`.

"Entsperren verweigert" mit dem Verweis auf das Server-Log bedeutet,
dass die Pfade im Archiv nicht zur Datenbank passen. Klein.Buch prüft beim
Entsperren einige Archiveinträge; liegen sie nicht unter
`/data/archive`, gehört der Datenordner nicht zu dieser Datenbank.

"Entsperren abgebrochen: Die Prüfung der Datenbank hat Unstimmigkeiten
gefunden" bedeutet, dass die Datenbank nicht in sich stimmig ist.
Klein.Buch hat dann nichts verändert und bleibt gesperrt. Im Log steht
eine Zeile `ERROR` mit "Datenbank-Prüfung beim Entsperren fehlgeschlagen"
und dem Grund:

- "verletzte(r) Fremdschlüssel": Ein Eintrag verweist auf einen Datensatz,
  den es nicht gibt, etwa nach einer Bearbeitung der Datenbank von Hand.
  Die Zeilen davor nennen Tabelle und Zeilennummer, aber keine Inhalte.
- "Migration 0032 abgebrochen: Zu mindestens einer Rechnung gibt es mehr
  als einen festgeschriebenen Storno-Beleg": Das Update hätte eine Regel
  eingeführt, gegen die Deine Daten schon verstoßen. Festgeschriebene
  Belege ändert Klein.Buch nie, auch nicht zur Reparatur.

Nach einem Update prüft Klein.Buch beides, bevor es die Datenbank auf das
neue Format umstellt, und auch vor dem Backup, das jeder Umstellung
vorausgeht. Steht in der Meldung "Die Datenbank steht unverändert auf
Schema …", ist also nichts umgestellt. Dann gilt: Setz wieder das Image
der Version ein, mit der die Datenbank zuletzt lief (Kapitel "Updates
und Rückfall"). Es öffnet die Datenbank wie vorher. Ohne Update, also
ohne anstehende Umstellung, gilt dasselbe: Die Daten sind unverändert.

Schlägt die Prüfung erst nach der Umstellung an, sagt die Meldung im
Browser "Nach der Umstellung der Datenbank hat die Prüfung
Unstimmigkeiten gefunden", und im Log steht "nach der Umstellung von
Schema …". Das ist ein Fehler der Umstellung selbst und sollte nicht
vorkommen. Klein.Buch meldet das bei jedem weiteren Anmeldeversuch
wieder so, bis ein Entsperren gelingt. Die Umstellung ist dann schon
gespeichert, das vorige Image öffnet die Datenbank nicht mehr, und ohne
Anmeldung lässt sich in dieser Instanz kein Backup einspielen. Der Rückweg führt über eine neue Instanz:

1. Kopiere das Backup, das Klein.Buch unmittelbar vor der Umstellung
   angelegt hat (die neueste `.kbk`-Datei im Backup-Ordner
   `/data/backups`), an einen sicheren Ort, ebenso den ganzen Ordner
   `/data` als Sicherung.
2. Richte mit dem Image der vorigen Version und einem leeren Ordner für
   `/data` eine neue Instanz ein und spiel dieses Backup ein (Kapitel
   "Notfall: Instanz neu aufsetzen und wiederherstellen").
3. Wende Dich mit den Log-Zeilen an den Support, bevor Du das Update
   erneut versuchst.

Lässt sich die Ursache in keinem der Fälle klären, wende Dich mit den
Log-Zeilen an den Support.

Lehnt Klein.Buch einen Upload mit dem Hinweis auf zu wenig Platz ab
(HTTP-Status 507), liegt der freie Platz in `/data` unter
`KLEIN_BUCH_MIN_FREE_BYTES` (Standard 2 GiB) plus der Größe der Datei.
Scheitert ein großer Upload mit Status 413 ("zu groß") und steht im Log
von Klein.Buch dazu keine Zeile `POST /api/upload/…`, hat schon der
Proxy die Datei abgewiesen; seine Upload-Grenze ist zu klein.

Nach einer Wiederherstellung beendet sich Klein.Buch absichtlich und
wartet auf den Neustart durch Docker; erst dieser Neustart spielt das
Backup ein. Startet der Container danach nicht von selbst, prüfe
`restart: unless-stopped` in der Compose-Datei.

Probleme mit der Prüfung und Erzeugung von E-Rechnungen beschreibt das
Kapitel "Sidecar-Probleme".

## Fehlversuche für fail2ban

Klein.Buch bremst Fehlversuche selbst (Kapitel "Zweiter Faktor (TOTP)
einrichten und wiederherstellen"). Wer zusätzlich Adressen auf der
Firewall sperren will, etwa mit fail2ban (einem Werkzeug, das Logs liest
und auffällige Adressen sperrt), findet jeden Fehlversuch als eigene
Zeile im Container-Log. Sie sieht so aus:

```
2026-10-24T09:15:02.123456Z  WARN klein_buch_lib::auth: auth.login.failed ip=203.0.113.7 reason=totp
```

Hinter dem Zeitstempel stehen zwei Leerzeichen. Es gibt drei Arten:
`auth.login.failed` (Anmeldung), `auth.setup.failed` (Einrichtung mit
Setup-Token) und `auth.reauth.failed` (erneute Bestätigung). `reason`
ist `totp` (Code oder Einmal-Code falsch), `passphrase` (Daten-Passwort
falsch), `token` (Setup-Token falsch) oder `rate_limited` (Versuch
während einer Wartezeit). `ip` ist die Adresse des Browsers, bei IPv6
in der üblichen Schreibweise. Sie stimmt nur, wenn
`KLEIN_BUCH_TRUSTED_PROXIES` richtig gesetzt ist und der Proxy
`X-Forwarded-For` schickt; sonst steht dort die Adresse des Proxys.
Fehlversuche stehen nur im Log, nicht im Audit-Protokoll von
Klein.Buch. Setzt Du `RUST_LOG` strenger als `warn`, fehlen diese
Zeilen.

Ein passender Filter für fail2ban, zum Beispiel als
`/etc/fail2ban/filter.d/klein-buch.conf`:

```
[Definition]
failregex = ^(?:\{"log":")?\S*\s+WARN klein_buch_lib::auth: auth\.(?:login|setup|reauth)\.failed ip=<HOST> reason=[a-z_]+(?:\\n"|$)
ignoreregex =
```

Der Filter ist am Zeilenanfang und am Zeilenende verankert und verlangt
das Ziel `klein_buch_lib::auth` direkt hinter dem Zeitstempel. So zählt
nur eine echte Fehlversuch-Zeile, nie ein Text, der zufällig in einer
anderen Zeile steht. Klein.Buch schreibt außerdem jedes Ereignis als
genau eine Zeile; Zeilenumbrüche in Werten erscheinen als `\n`. Der
Filter passt auf beide Formen: auf die Zeilen von `docker logs` und auf
die Dateien des Log-Treibers `json-file`. Ältere, nicht verankerte
Filter solltest Du durch diesen ersetzen.

Und eine Jail dazu, zum Beispiel `/etc/fail2ban/jail.d/klein-buch.local`:

```
[klein-buch]
enabled  = true
filter   = klein-buch
logpath  = /var/lib/docker/containers/*/*-json.log
maxretry = 5
findtime = 10m
bantime  = 1h
```

Mit dem Log-Treiber `json-file` aus der Beispiel-Compose-Datei legt
Docker das Log unter `/var/lib/docker/containers/<ID>/` ab. Die ID
ändert sich, wenn der Container neu angelegt wird, etwa bei einem
Update; lade fail2ban danach neu (`fail2ban-client reload`), damit es
die neue Datei liest. Läuft Dein Reverse-Proxy selbst als Container mit
veröffentlichten Ports, greift eine Sperre in der üblichen Kette der
Firewall nicht. Bei iptables gehört sie dann in die Kette `DOCKER-USER`
(in der Jail `chain = DOCKER-USER`).

---

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.10*
