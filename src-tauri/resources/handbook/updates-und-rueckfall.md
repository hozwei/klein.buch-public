---
slug: updates-und-rueckfall
title: Updates und Rückfall
category: betrieb
order: 50
keywords: [update, aktualisieren, version, image-tag, migration, rückfall, rollback, versionsprüfung, cosign, signatur]
---

# Updates und Rückfall

Klein.Buch aktualisiert sich nie selbst. Du entscheidest, wann eine neue
Version läuft, indem Du in Deiner Container-Verwaltung das Image-Tag
änderst, also die Versionsnummer hinter dem Doppelpunkt in
`ghcr.io/hozwei/klein-buch:2026.11.1`. Einen Update-Knopf in der App
gibt es absichtlich nicht: Dafür bräuchte der Container Zugriff auf
Docker selbst und damit praktisch volle Rechte über Deinen Server. Bei
einem Dienst, der aus dem Internet erreichbar ist, wäre das zu
gefährlich.

## Von neuen Versionen erfahren

Unter "Einstellungen" und "Sicherheit & Anmeldung" kannst Du die
Versionsprüfung einschalten ("Nach neuen Versionen suchen"). Sie ist
ab Werk aus. Eingeschaltet fragt Klein.Buch höchstens einmal am Tag bei
GitHub nach der neuesten Veröffentlichung, und zwar nur, solange es
entsperrt ist. Gesendet wird eine einzige Anfrage mit dem festen
Programmnamen `Klein.Buch-Versionspruefung`, ohne Deine Version, ohne
Cookie und ohne Angaben über Deine Instanz. Gibt es eine neuere Version,
legt Klein.Buch einen Eintrag unter "Hinweise" an, mit Name und Link als
reinem Text, je Version einmal. Vorabversionen zählen nicht.

Gefragt wird bei den Veröffentlichungen des Projekts, aus dem Deine
Version stammt. Mit `KLEIN_BUCH_UPDATE_URL` kannst Du eine andere,
vollständige Abfrageadresse vorgeben, etwa einen eigenen Spiegel; sie
muss mit `https://` oder `http://` beginnen, sonst gilt sie nicht. Der
Container braucht für die Versionsprüfung eine ausgehende
HTTPS-Verbindung.

Ohne Versionsprüfung schaust Du selbst unter "Releases" im öffentlichen
Projekt `https://github.com/hozwei/klein.buch-public` nach. Dort steht
zu jeder Version, was sich geändert hat.

## Ein Update einspielen

1. Lies die Änderungen der neuen Version und aller übersprungenen
   Versionen. Du kannst Versionen überspringen; nötige Umstellungen
   der Datenbank laufen dann nacheinander in einem Schritt.
2. Prüfe unter "Einstellungen" und "Backups" im Backup-Protokoll, dass
   das letzte Backup auf Deinem externen Ziel geklappt hat. Am
   sichersten legst Du direkt vor dem Update mit der alten Version noch
   ein Backup an ("Jetzt sichern"): Das kann die alte Version bei einem
   Rückfall in jedem Fall einspielen.
3. Wenn Du willst, prüfe die Signatur des neuen Images (siehe unten).
4. Trag die neue Nummer in der Compose-Datei ein, zum Beispiel
   `image: ghcr.io/hozwei/klein-buch:2026.11.1`.
5. Lade das Image und starte den Container neu:

```
docker compose pull
docker compose up -d
```

Docker ersetzt dabei den Container. Klein.Buch hält zuerst seine
geplanten Aufgaben an; angefangene Aufgaben dürfen bis zu 30 Sekunden zu
Ende laufen, und ein noch ausstehendes Backup eines eben
festgeschriebenen Belegs wird noch geschrieben (zusammen höchstens
40 Sekunden, innerhalb der `stop_grace_period` von 90 Sekunden). Danach
ist Klein.Buch gesperrt, bis Du Dich anmeldest.

Bringt die neue Version eine Umstellung der Datenbank mit (eine
Migration), passiert sie bei Deiner ersten Anmeldung. Vorher schreibt
Klein.Buch ein Backup des alten Stands, liest es vollständig zurück und
prüft es. Erst wenn das gelungen ist, stellt es die Datenbank um. Bei
einem großen Archiv dauert das eine Weile; die Anmeldemaske wartet, bis
es fertig ist (auch über Zeitlimits des Reverse-Proxys hinweg). Im
Backup-Protokoll erscheint dieses Backup als manuell ausgelöst; es wird
nie automatisch gelöscht. Scheitert das Backup, etwa weil der Platz
nicht reicht, stellt Klein.Buch nichts um und bleibt gesperrt. Du siehst
dann "Entsperren abgebrochen: Vor der Datenbank-Umstellung ließ sich kein
Backup schreiben."; die Einzelheiten stehen im Container-Log (Zeile
"Pflicht-Backup vor der Datenbank-Migration fehlgeschlagen"). Schaffe
Platz oder korrigiere die Rechte von `/data` und melde Dich erneut an.

Bringt das Image neue Fassungen der Vorlagen in `/inputs` mit, gleicht
Klein.Buch sie beim Containerstart ab und meldet das unter "Hinweise"
(Kapitel "Volumes und Rechte"). Eine `.neu`-Datei neben einer von Dir
geänderten Vorlage legt es je Fassung nur einmal an; Du darfst sie nach
dem Vergleich löschen.

## Signatur prüfen

Jedes veröffentlichte Image ist signiert. Die Signatur beweist, dass das
Image von der Veröffentlichungs-Pipeline des öffentlichen Projekts auf
GitHub gebaut wurde und unterwegs nicht verändert wurde. Prüfen kannst
Du sie mit dem Werkzeug cosign:

```
cosign verify ghcr.io/hozwei/klein-buch:<v> --certificate-identity-regexp '^https://github\.com/hozwei/klein\.buch-public/\.github/workflows/release-image\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$' --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

Ersetze `<v>` durch die Version, zum Beispiel `2026.11.1`. Meldet cosign
einen Fehler, starte dieses Image nicht. Wer ganz sichergehen will,
schreibt zusätzlich den Digest (die Prüfsumme des Images) aus den
Release-Notizen hinter die Version in die Compose-Datei.

## Zurück zur vorherigen Version

Images und Versionsnummern werden nie gelöscht. Die vorherige Version
bleibt also immer abrufbar. Wie Du zurückkommst, hängt davon ab, ob Du
Dich mit der neuen Version schon angemeldet hast.

Hast Du Dich noch nicht angemeldet, hat sich an der Datenbank nichts
geändert. Trag die alte Nummer wieder ein und starte mit
`docker compose up -d` neu.

Hast Du Dich angemeldet und die neue Version hat die Datenbank
umgestellt, öffnet die alte Version sie nicht mehr. Klein.Buch prüft
beim Entsperren die Version der Datenbank und verweigert eine, die von
einer neueren Version stammt. Das schützt Deine Daten vor einer
Version, die sie nicht versteht. Der Weg zurück führt dann über das
Backup, das Klein.Buch vor der Umstellung geschrieben hat:

1. Stoppe den Container und benenne den Ordner von `/data` um, zum
   Beispiel in `data-neu`. Lösch ihn nicht; er enthält alles, was Du nach
   dem Update erfasst hast.
2. Such das Backup von vor der Umstellung heraus. Es liegt im
   umbenannten Ordner unter `backups/` (Dateiname
   `klein-buch-<Zeitpunkt>-<Kennung>.kbk`, Zeitpunkt kurz vor Deiner
   ersten Anmeldung nach dem Update) und auf Deinem externen Ziel.
   Klein.Buch schreibt es so, dass die vorherige Version es lesen kann.
   Spätere Backups der neuen Version liest die alte unter Umständen
   nicht (sie meldet dann "Backup-Format … wird nicht unterstützt").
3. Leg einen neuen, leeren Ordner für `/data` an, gib ihn dem
   Klein.Buch-Benutzer und trag die alte Versionsnummer ein.
4. Starte Klein.Buch, richte es mit dem neuen Setup-Token ein und spiel
   das Backup per Upload ein. Die einzelnen Schritte stehen im Kapitel "Notfall: Instanz neu aufsetzen und wiederherstellen". Danach meldest Du Dich mit dem Daten-Passwort
   des Backups an.

Alles, was Du nach dem Update erfasst hast, fehlt in diesem Backup. Hast
Du nach dem Update schon Belege festgeschrieben, geh diesen Weg nicht:
Die alte Version würde deren Nummern ein zweites Mal vergeben, und die
festgeschriebenen Belege fehlten in Deiner Buchführung. Bleib dann auf
der neuen Version, bis eine korrigierte erscheint, und bewahre den
Ordner `data-neu` in jedem Fall auf.

Ein Backup einer älteren Version kannst Du auch in der neuen Version
einspielen; Klein.Buch stellt es danach auf den neuen Stand um. Das hilft
bei verlorenen Daten, aber nicht beim Zurückgehen. Ein Backup einer
neueren Version nimmt eine ältere dagegen nie an.

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
