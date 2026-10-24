---
slug: installation-docker
title: Installation mit Docker Compose
category: betrieb
order: 10
keywords: [installation, docker, compose, container, image, setup-token, einrichtung, betrieb, server, volume]
---

# Installation mit Docker Compose

Klein.Buch läuft auf Deinem eigenen Server als Container. Ein Image
ist ein fertig gepacktes Programm mit allem, was es zum Laufen
braucht. Ein Container ist ein laufendes Exemplar dieses Images.
Docker ist das Werkzeug, das Container startet und verwaltet, und
Docker Compose beschreibt in einer einzigen Datei, wie der Container
starten soll. Dieses Kapitel richtet sich an Dich als Betreiber, also
an die Person, die den Server einrichtet. Eine Klein.Buch-Instanz
gehört genau einem Unternehmen; weitere Benutzerkonten gibt es nicht.

## Was Du vorher brauchst

Du brauchst einen Linux-Server mit 64-Bit-Intel- oder AMD-Prozessor
(das Image gibt es nur für `linux/amd64`), auf dem Docker und das
Compose-Plugin installiert sind. Dazu kommt eine eigene Adresse nur
für Klein.Buch, zum Beispiel `buch.example.org`, und ein Reverse-Proxy
mit gültigem Zertifikat davor. Wie das zusammenspielt, steht im Kapitel
"Reverse-Proxy und TLS". Ohne HTTPS lässt Klein.Buch weder die
Einrichtung noch eine Anmeldung zu.

Für die Daten brauchst Du zwei Ordner auf einer lokalen Festplatte des
Servers, nicht auf einer Netzwerkfreigabe. Warum, erklärt das Kapitel
"Volumes und Rechte". Außerdem brauchst Du mindestens ein Ziel für
Sicherungen außerhalb des Servers: einen Ordner auf einem anderen
Gerät, den Du unter `/backup-targets` einbindest, oder einen
SFTP-Server (Kapitel "Off-Site-Backup einrichten"). Für den
Arbeitsspeicher nennt das Beispiel unten 2 GiB; wie viel Du wirklich
brauchst, hängt von der Größe Deiner Backups ab (Kapitel "Speicher und Ressourcen"). Und Du brauchst eine Authenticator-App auf Deinem
Handy für den zweiten Faktor.

## Die Beispiel-Datei

Im öffentlichen Projekt unter `https://github.com/hozwei/klein.buch-public`
liegt die Datei `compose.example.yaml`. Kopiere sie auf Deinen Server
als `compose.yaml` und passe Adresse, Pfade und Benutzer an. Ihr Kern
sieht so aus:

```yaml
services:
  klein-buch:
    image: ghcr.io/hozwei/klein-buch:VERSION
    container_name: klein-buch
    restart: unless-stopped
    user: "1000:1000"
    environment:
      KLEIN_BUCH_PUBLIC_URL: "https://buch.example.org"
      KLEIN_BUCH_TRUSTED_PROXIES: "198.51.100.10/32"
    volumes:
      - /srv/klein-buch/data:/data
      - /srv/klein-buch/inputs:/inputs
      - /mnt/nas/backups:/backup-targets/nas
    ports:
      - "127.0.0.1:8080:8080"
    read_only: true
    tmpfs:
      - /tmp:size=512m,mode=1777,noexec,nosuid,nodev
    cap_drop: [ALL]
    security_opt:
      - "no-new-privileges:true"
    ulimits:
      core: 0
    pids_limit: 256
    mem_limit: 2g
    stop_grace_period: 90s
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "5"
```

## Was die Zeilen bedeuten

`image` nennt das Image samt Version. Ersetze `VERSION` durch die
Nummer einer Veröffentlichung, zum Beispiel `2026.10.0`. Die
Versionen findest Du unter "Releases" im öffentlichen Projekt. Ein
Image mit dem Namen `latest` gibt es absichtlich nicht: Ein
wandernder Name würde bei jedem Herunterladen unbemerkt eine neue
Version holen, die Deine Datenbank umstellt. Mit einer festen Nummer
ist jedes Update ein bewusster Schritt, und die alte Version bleibt als
Rückfall verfügbar (Kapitel "Updates und Rückfall").

`container_name` gibt dem Container einen festen Namen. Die Befehle in
diesem Handbuch benutzen `klein-buch`.

`restart: unless-stopped` startet den Container nach einem Absturz und
nach einem Neustart des Servers wieder. Das ist mehr als Bequemlichkeit:
Nach einer Wiederherstellung aus einem Backup beendet sich Klein.Buch
absichtlich und verlässt sich darauf, dass Docker es neu startet; erst
dieser Neustart spielt das Backup ein. Eine Einstellung wie
`on-failure` startet nach einem geordneten Ende nicht neu. Nimm also
`unless-stopped` oder `always`.

`user` legt fest, unter welcher Benutzer- und Gruppennummer (UID und
GID) Klein.Buch läuft. Ohne diese Zeile läuft das Image als
`65532:65532`. Die eingebundenen Ordner müssen diesem Benutzer
gehören. Als `root` (Nummer 0) startet Klein.Buch nicht.

`environment` enthält die Einstellungen. Pflicht ist nur
`KLEIN_BUCH_PUBLIC_URL`, die öffentliche Adresse, unter der Du
Klein.Buch im Browser öffnest. `KLEIN_BUCH_TRUSTED_PROXIES` nennt die
Adresse Deines Reverse-Proxys. Beides erklärt das Kapitel
"Reverse-Proxy und TLS".

`volumes` blendet Ordner des Servers in den Container ein: links der
Ordner auf dem Server, rechts der feste Ort im Container. `/data`
enthält Datenbank, Beleg-Archiv und lokale Backups, `/inputs` die
Vorlagen, und jeder Ordner unter `/backup-targets/` ist ein externes
Backup-Ziel. Den Namen hinter `/backup-targets/` wählst Du selbst.

`ports` macht Port 8080 des Containers nur auf dem Server selbst
erreichbar (`127.0.0.1`), damit der Reverse-Proxy auf demselben Server
ihn ansprechen kann. Gib Port 8080 nie direkt ins Internet frei: Im
Container gibt es keine Verschlüsselung.

Die folgenden Zeilen härten den Container. `read_only` macht das
Dateisystem des Containers schreibgeschützt; Klein.Buch schreibt nur in
die Volumes und nach `/tmp`. Eigene Zwischendateien legt es unter
`/data/tmp` ab, nicht in `/tmp`. `tmpfs` stellt `/tmp` als kleinen
Bereich im Arbeitsspeicher bereit. `cap_drop: [ALL]` und
`no-new-privileges` nehmen dem Container alle Sonderrechte des
Betriebssystems. `ulimits: core: 0` verhindert Speicherabbilder bei
einem Absturz, die Dein Daten-Passwort enthalten könnten. `pids_limit`
und `mem_limit` begrenzen Prozesse und Arbeitsspeicher.

`stop_grace_period: 90s` gibt Klein.Buch beim Stoppen bis zu 90
Sekunden Zeit. Klein.Buch hält zuerst seine geplanten Aufgaben an (ein
Abo-Lauf beendet den gerade bearbeiteten Termin), wartet höchstens 30
Sekunden auf offene Verbindungen und weitere 30 Sekunden auf angefangene
Aufgaben wie ein Festschreiben oder ein Backup und schließt dann die
Datenbank. Docker wartet sonst nur 10 Sekunden und beendet den Prozess
dann hart. Verkürze diesen Wert nicht. Ein abgebrochenes Backup ist
unkritisch: Es wird erst unter einem Zwischennamen geschrieben und erst
am Ende umbenannt; Reste räumt Klein.Buch beim nächsten Start auf.

`logging` begrenzt das Container-Log auf fünf Dateien zu je 10 MB. Im
Log stehen der Setup-Token und fehlgeschlagene Anmeldungen mit
IP-Adresse, aber nie Dein Daten-Passwort.

Eine Prüfung, ob Klein.Buch läuft (Healthcheck), ist im Image schon
eingebaut. Du musst sie nicht in die Compose-Datei schreiben.

## Weitere Einstellungen

Die Pfade im Container (`KLEIN_BUCH_DATA_DIR`, `KLEIN_BUCH_INPUTS_DIR`,
`KLEIN_BUCH_RESOURCE_DIR`, `KLEIN_BUCH_WEB_DIR`) setzt das Image selbst.
Ändere sie nicht. Alles Weitere ist freiwillig:

- `KLEIN_BUCH_TRUSTED_PROXIES`: Adressen der vertrauten Reverse-Proxys,
  durch Komma oder Leerzeichen getrennt. Standard: leer.
- `KLEIN_BUCH_LISTEN`: Adresse und Port im Container in der Form
  `IP:Port`. Standard: `0.0.0.0:8080`.
- `KLEIN_BUCH_BACKUP_TARGETS_DIR`: Wurzel der Verzeichnis-Backup-Ziele.
  Standard: `/backup-targets`.
- `KLEIN_BUCH_MIN_FREE_BYTES`: so viel Platz in Byte muss frei bleiben,
  sonst lehnt Klein.Buch Uploads ab. Standard: 2 GiB (`2147483648`).
- `KLEIN_BUCH_MAX_RESTORE_BYTES`: größte Datenmenge in Byte, die eine
  Wiederherstellung auspacken darf. Standard: 4 GiB (`4294967296`).
- `KLEIN_BUCH_ALLOWED_TEST_PORTS`: Ports für die Verbindungstests von
  Mail und SFTP, durch Komma getrennt. Ohne Angabe gelten 25, 465, 587
  und 2525 für Mail und 22 und 2222 für SFTP.
- `KLEIN_BUCH_SMTP_HELO_NAME`: Name, mit dem sich Klein.Buch beim
  Mailserver meldet.
- `KLEIN_BUCH_SMTP_CA_FILE`: Pfad zu einer eigenen
  Zertifizierungsstelle (PEM-Datei in einem Volume) für Deinen
  Mailserver.
- `KLEIN_BUCH_SOURCE_URL`: Adresse Deines eigenen Quellcodes, wenn Du
  Klein.Buch veränderst (Kapitel "Lizenz und Quellcode: wenn Du Klein.Buch veränderst").
- `KLEIN_BUCH_UPDATE_URL`: andere Abfrageadresse für die
  Versionsprüfung (Kapitel "Updates und Rückfall").
- `RUST_LOG`: wie ausführlich das Log ist. Standard: `info`.

Einen ungültigen Wert bei öffentlicher Adresse, Proxys, `KLEIN_BUCH_LISTEN`,
der Wurzel der Backup-Ziele, den beiden Platzgrenzen oder den Test-Ports
meldet Klein.Buch beim Containerstart im Log und startet nicht. Zwei Schalter gibt es nur für Tests, nie für den Betrieb:
`KLEIN_BUCH_INSECURE_HTTP=1` erlaubt die Anmeldung ohne HTTPS, und
`KLEIN_BUCH_ALLOW_UNMOUNTED=1` schaltet die Prüfung der Volumes ab.

## Der erste Start

Lege zuerst die Ordner an und gib sie dem Benutzer aus `user`:

```
sudo mkdir -p /srv/klein-buch/data /srv/klein-buch/inputs
sudo chown 1000:1000 /srv/klein-buch/data /srv/klein-buch/inputs
```

Für jedes Backup-Ziel gilt dasselbe. Dann startest Du Klein.Buch im
Ordner mit der `compose.yaml`:

```
docker compose up -d
docker compose logs klein-buch
```

Beim ersten Start füllt Klein.Buch `/inputs` mit den mitgelieferten
Vorlagen und schreibt einen Setup-Token ins Container-Log. Der Token
ist eine einmalige Zeichenfolge, mit der Du beweist, dass Du Zugriff
auf den Server hast. Im Log steht er als eigener Block:

```
=== Klein.Buch: Einrichtung nötig ===
Setup-Token: XXXXXXXXXXXXXXXXXXXXXXXXXX
```

Der Token gilt bis zum nächsten Neustart des Containers. Startest Du
neu, bevor die Einrichtung fertig ist, steht ein neuer Token im Log.
In der Oberfläche von Klein.Buch erscheint er nie.

## Einrichtung im Browser

Öffne die Adresse aus `KLEIN_BUCH_PUBLIC_URL`, im Beispiel
`https://buch.example.org`. Klein.Buch zeigt "Klein.Buch einrichten".

1. Trag den Setup-Token ein.
2. Leg Dein Daten-Passwort fest (mindestens 16 Zeichen, am besten drei
   oder vier zufällige Wörter) und wiederhole es. Das Daten-Passwort ist
   die Passphrase aus dem Glossar: Es verschlüsselt Datenbank und
   Backups. Es wird nirgends gespeichert und lässt sich nicht
   zurücksetzen. Mehr dazu im Kapitel "Passphrase einrichten".
3. Klick auf **"Weiter zum QR-Code"**, scanne den QR-Code mit Deiner
   Authenticator-App und gib den angezeigten Code ein.
4. Klick auf **"Einrichten & erstes Backup erstellen"**.
5. Notiere die zehn Einmal-Codes, hake "Ich habe die Codes sicher
   notiert." an und klick auf "Weiter".

Danach richtest Du Deine Firmendaten und ein Off-Site-Backup ein
(Kapitel "Off-Site-Backup einrichten"). Solange es kein externes Ziel
gibt, erinnert Klein.Buch Dich daran.

Merke Dir eine Eigenheit des Betriebs: Nach jedem Neustart des
Containers ist Klein.Buch gesperrt, bis Du Dich im Browser anmeldest.
Erst dann laufen Tages-Backup und andere geplante Aufgaben weiter.
Warum das so ist, steht im Kapitel "Fehlersuche im Container".

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
