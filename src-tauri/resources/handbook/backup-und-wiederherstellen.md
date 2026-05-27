---
slug: backup-und-wiederherstellen
title: Backup und Wiederherstellen
category: bedienen
order: 220
keywords: [backup, restore, wiederherstellen, off-site, sftp, retention, protokoll]
---

# Backup und Wiederherstellen

Klein.Buch sichert Deine Daten automatisch. Du brauchst Dich um
das Backup im Alltag nicht zu kümmern. Wenn doch einmal etwas
zurückgespielt werden muss, gibt es die Wiederherstellungs-Funktion.

## Backups anschauen

Klick im Hauptmenü auf "Einstellungen", dann auf "Backups". Du
siehst eine Liste pro Backup-Ziel mit Datum, Größe und Auslöser
(zum Beispiel "Auto nach Festschreibung", "Täglich", "Vor
Wiederherstellen"). Daneben gibt es einen Knopf, um den Ordner zu
öffnen,
falls Du das Backup von Hand kopieren willst.

## Aufbewahrung

Klein.Buch hält mehrere Generationen vor und sortiert ältere
automatisch aus.

**Lokale Sicherung** (immer aktiv, im App-Daten-Ordner): die
letzten 7 Tages-Backups, die letzten 3 Monats-Backups, das letzte
Jahres-Backup.

**Off-Site-Ziel** (Cloud-Ordner oder SFTP, separat eingerichtet):
die letzten 30 Tages-Backups, die letzten 12 Monats-Backups, die
letzten 7 Jahres-Backups.

Das jeweils neueste Backup wird nie gelöscht, auch wenn die
Retention-Mengen überschritten sind. Das verhindert, dass eine zu
enge Aufbewahrungs-Einstellung Dich vollständig ohne Sicherung
zurücklässt.

## Manuelles Backup auslösen

Klick auf "Jetzt Backup machen". Klein.Buch erzeugt sofort ein
neues Backup auf allen aktiven Zielen und protokolliert das
Ergebnis. Sinnvoll vor größeren Änderungen (Verkäuferprofil-Umbau,
Import eines fremden Datenbestands, Hardware-Wechsel).

## Wiederherstellen

So läuft der Ablauf:

1. Auf der Backups-Seite in der Liste auf "Auswählen" neben dem
   gewünschten Backup klicken.
2. Klein.Buch zeigt eine Vorschau (Datum, Größe, Auslöser).
3. Das Backup-Passwort eingeben, das beim Erstellen dieses Backups
   gültig war.
4. Auf "Wiederherstellen" klicken und bestätigen.
5. Klein.Buch erzeugt ein Sicherheits-Backup des aktuellen Stands
   (mit Auslöser "Vor einer Wiederherstellung"), schreibt einen
   Wiederherstellungs-Marker und beendet sich.
6. **Du startest Klein.Buch von Hand neu.** Erst beim Neustart
   wird der Daten-Ordner mit dem gewählten Backup-Stand
   überschrieben.

Wenn Du das Passwort zwischenzeitlich geändert hast, brauchst Du
das **alte** Passwort (zum Entschlüsseln des Backups). Das
Sicherheits-Backup vor der Wiederherstellung erzeugt Klein.Buch mit
Deinem **aktuellen** Passwort, weil das die aktuelle Session-
Verschlüsselung ist.

## Rückzug auf den vorherigen Stand

Wenn die Wiederherstellung nicht das Erwartete ergeben hat (zum
Beispiel weil Du das falsche Backup gewählt hast), kannst Du
sofort das Sicherheits-Backup von vorher zurückspielen. Es liegt
auf jedem Ziel mit dem Auslöser "Vor Wiederherstellen".
Klein.Buch listet es immer ganz oben in der Backup-Liste.

## Verschlüsselungs-Konsistenz

Backups sind mit Deiner aktuellen Passphrase verschlüsselt. Wenn
Du die Passphrase wechselst, verschlüsselt Klein.Buch neue
Backups mit dem neuen Schlüssel. Alte Backups (vor dem Wechsel)
bleiben mit dem alten Schlüssel verschlüsselt. Bewahre den alten
Eintrag in Deinem Passwort-Manager so lange auf, wie Du auf alte
Backups zugreifen können willst.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
