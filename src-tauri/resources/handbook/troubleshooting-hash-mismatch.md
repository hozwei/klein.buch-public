---
slug: troubleshooting-hash-mismatch
title: Hash-Mismatch im Archiv
category: troubleshooting
order: 540
keywords: [hash, mismatch, archiv, integrität, manipulation, prüfsumme, sha-256]
---

# Hash-Mismatch im Archiv

Klein.Buch legt jede ausgestellte Rechnung und jeden Beleg als Datei im
Beleg-Archiv auf dem Server ab. Beim Ablegen berechnet es eine
Prüfsumme (SHA-256) und speichert sie in der Datenbank. Eine Prüfsumme
ist eine Art Fingerabdruck des Datei-Inhalts: Ändert sich auch nur ein
einziges Zeichen, ändert sich die Prüfsumme. Bei jedem Öffnen oder
Herunterladen einer Archiv-Datei und bei der monatlichen
Archiv-Prüfung rechnet Klein.Buch die Prüfsumme neu aus. Passt sie
nicht mehr zum gespeicherten Wert, nennt man das einen Hash-Mismatch
(Hash ist der Fachbegriff für die Prüfsumme, Mismatch heißt
Nichtübereinstimmung).

## Wie Du davon erfährst

Findet die Archiv-Prüfung einen Schaden, erscheint unter "Hinweise" der
dringende Eintrag "Archiv-Integrität gestört". Er nennt, wie viele
Dateien verändert sind und wie viele fehlen. Der Knopf "Öffnen" führt zu
"Einstellungen" und "Protokoll & Datensicherheit".

Dort zeigt die Karte "Archiv-Integrität" jede bisherige Prüfung mit
Zeitpunkt und den Spalten "Geprüft", "Ok", "Fehler", "Davon fehlend" und
"Nicht lesbar". Hatte eine Prüfung Befunde, klappst Du in der Spalte
"Betroffene Dateien" die Liste auf ("… Dateien anzeigen"). Sie nennt je
Datei die Art des Befunds ("Prüfsumme weicht ab", "fehlt" oder "nicht
lesbar"), die Archiv-ID und, wenn bekannt, den Dateinamen. Mit "Jetzt
prüfen" startest Du die Prüfung sofort; während sie läuft, heißt der
Knopf "Prüfe …", danach meldet Klein.Buch das Ergebnis. Automatisch
läuft sie einmal im Monat, allerdings nur, solange Klein.Buch entsperrt
ist.

Darunter steht das Audit-Protokoll mit den Spalten "Zeitpunkt",
"Aktion", "Ausgelöst von" ("Du", "Zeitplan" oder "System") und
"Bezug". Befunde stehen dort als "Integrität gestört (Prüfsumme weicht
ab)", "Archiv-Datei fehlt" oder "Archiv-Datei nicht lesbar", in der
Regel mit dem Namen der betroffenen Datei.

Willst Du eine betroffene Datei öffnen, liefert Klein.Buch sie nicht aus,
sondern meldet einen Fehler. Es gibt Dir also nie unbemerkt eine
veränderte Datei heraus.

## Was das bedeutet

Klein.Buch unterscheidet drei Fälle.

Inhalt verändert: Die Datei ist noch da, aber ihr Inhalt passt nicht
mehr zur Prüfsumme. Das ist der ernste Fall.

Datei fehlt: Die Datei ist nicht mehr im Archiv. Das deutet auf ein
versehentliches Löschen oder Verschieben.

Datei nicht lesbar: Die Datei ist da, aber Klein.Buch darf sie nicht
lesen, meist wegen falscher Dateirechte. Klein.Buch meldet das getrennt
mit dem Hinweis "Archiv-Dateien nicht lesbar". Das ist kein Hinweis auf
eine Manipulation, die Prüfung bleibt aber unvollständig. Der Betreiber
korrigiert die Rechte im Daten-Ordner (siehe Kapitel "[Volumes und
Rechte](/help/volumes-und-rechte)").

Klein.Buch selbst verändert abgelegte Archiv-Dateien nie. Es fügt nur
neue hinzu. Eine Veränderung muss also von außen kommen.

## Mögliche Ursachen

1. Jemand hat auf dem Server direkt im Daten-Ordner gearbeitet und
   Dateien im Archiv geöffnet, verschoben, gelöscht oder neu gespeichert.
2. Ein anderes Programm auf dem Server schreibt in den Daten-Ordner,
   zum Beispiel ein Sync- oder Kopier-Programm, das alte Stände
   zurückschreibt.
3. Der Daten-Ordner wurde von Hand umgezogen oder zurückkopiert, und
   dabei ist etwas schiefgegangen.
4. Ein Fehler des Speichers hat einzelne Bytes verändert.
5. Ein bösartiger Eingriff (Manipulation).

## Schreibschutz auf dem Server

Klein.Buch setzt jede Archiv-Datei nach dem Schreiben auf "nur lesen".
Auf einem Linux-Server schützt das aber nur vor versehentlichem
Überschreiben. Löschen und Umbenennen bleiben für denselben Benutzer
möglich, und ein Administrator des Servers kann ohnehin alles. Deshalb
gibt es die Prüfsumme in der Datenbank, die monatliche Prüfung und die
verschlüsselten Backups auf externen Zielen. Zusätzlich empfiehlt es
sich, dass der Betreiber regelmäßige Snapshots des Speichers anlegt
(Momentaufnahmen, aus denen sich ein früherer Stand zurückholen lässt).

## Was Du tun musst

Es gibt in Klein.Buch keinen Knopf, der die Prüfsumme neu berechnet oder
den Beleg "akzeptiert". Das wäre ein Verstoß gegen die GoBD (Grundsätze
ordnungsmäßiger Buchführung, die Regeln, nach denen das Finanzamt
Buchführungs-Software akzeptiert) und ist absichtlich nicht eingebaut.
Der einzige saubere Weg ist, ein Backup zurückzuspielen, das vor dem
Schaden entstanden ist.

Eine Wiederherstellung setzt die Datenbank und das gesamte Archiv auf
den Stand des Backups zurück. Belege, die Du seitdem erfasst hast, sind
danach nicht mehr in Klein.Buch. Vor der Wiederherstellung legt
Klein.Buch automatisch ein Sicherheits-Backup des aktuellen Stands an,
in dem diese Belege noch stehen.

So gehst Du vor:

1. Stell fest, seit wann der Schaden besteht. In der Karte
   "Archiv-Integrität" siehst Du, bei welcher Prüfung zuerst Fehler
   auftraten.
2. Bitte den Betreiber, den Daten-Ordner zusätzlich von Hand an einen
   sicheren Ort zu kopieren, damit nichts unbeabsichtigt verloren geht.
3. Spiel das letzte Backup vor dem Schaden zurück (siehe Kapitel
   "[Backup und Wiederherstellen](/help/backup-und-wiederherstellen)").
4. Prüfe mit "Jetzt prüfen", ob das Archiv jetzt einwandfrei ist.
5. Trag die Belege seit dem Backup-Zeitpunkt erneut ein. Geht es dabei um
   festgeschriebene Rechnungen, sprich vorher mit Deiner Steuerberatung.
   Klein.Buch ist ein Werkzeug, kein Steuerberater.

## Vorbeugen

1. Im Daten-Ordner auf dem Server nichts von Hand ändern.
2. Den Daten-Ordner nicht mit Sync-Programmen abgleichen. Die Kopie nach
   außen läuft über die Backup-Ziele von Klein.Buch.
3. Die Archiv-Prüfung unter "Protokoll & Datensicherheit" ab und zu von
   Hand starten, damit Du Probleme früh entdeckst.

## Bei wiederholten Fehlern

Tritt das Problem mehrfach auf, greift höchstwahrscheinlich ein Programm
oder eine Person auf dem Server regelmäßig auf den Daten-Ordner zu.
Such die Ursache gemeinsam mit dem Betreiber und stell sie ab. Sonst
entsteht eine Lücke in Deiner Belegaufzeichnung, die bei einer Prüfung
durch das Finanzamt auffallen kann.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
