---
slug: hinweise
title: Hinweise
category: bedienen
order: 210
keywords: [hinweise, inbox, benachrichtigung, erinnerung, notification, regel]
---

# Hinweise

Klein.Buch sammelt Mitteilungen in einer Hinweis-Liste (in
älteren Texten auch "Inbox" genannt). Dort siehst Du, was die App
im Hintergrund für Dich gemerkt hat: fälliges Backup, eingegangene
E-Rechnung, fehlgeschlagener Mail-Versand, abgelaufenes
OAuth-Token, anstehende AfA-Läufe, Abo-Rechnungs-Fälligkeiten.

## Liste öffnen

Klick im Hauptmenü auf "Hinweise". Du siehst eine Liste mit
ungelesenen Einträgen oben und gelesenen weiter unten. Jeder
Eintrag hat ein Datum, eine kurze Beschreibung und gegebenenfalls
einen Knopf für die nächste Aktion (zum Beispiel "Backup jetzt
machen" oder "OAuth neu anmelden").

Klick auf einen Eintrag, um die Details zu sehen. Klick auf "Als
gelesen markieren", um ihn nach unten zu schieben. Die gelesenen
Einträge bleiben sichtbar, damit Du im Nachhinein nachvollziehen
kannst, was wann passiert ist.

## Regeln

Welche Ereignisse einen Hinweis erzeugen sollen, stellst Du nicht
in den Einstellungen ein, sondern direkt in der Hinweis-Liste:
Klick im Hauptmenü auf "Hinweise" und dann oben rechts auf
"Erinnerungen einstellen".
Beispiele:

1. Backup-Überfälligkeit (Schwelle in Tagen)
2. Backup-Ergebnis (Erfolg, Fehler oder beides)
3. E-Rechnungs-Empfang
4. Mail-Versand-Fehler
5. OAuth-Token-Ablauf
6. Abo-Rechnungs-Fälligkeit

Jede Regel hat einen Schalter "An / Aus" und je nach Regel zusätzliche
Optionen (zum Beispiel Schwellen-Tage).

## OS-Benachrichtigungen

Klein.Buch kann zusätzlich zum Eintrag in der Hinweis-Liste auch
eine echte Windows-Toast-Benachrichtigung anzeigen. Schalte das je Regel ein
oder aus. Die OS-Benachrichtigungen sind sinnvoll bei wirklich
zeitkritischen Ereignissen (zum Beispiel Mail-Versand-Fehler) und
störend bei Routine-Ereignissen (zum Beispiel täglicher
Backup-Erfolg).

## Dedup

Klein.Buch erkennt, wenn dieselbe Bedingung mehrfach auftritt, und
fasst gleichartige Hinweise zu einem zusammen. Du bekommst nicht
20 separate Einträge, wenn ein OAuth-Token 20 Mal hintereinander
abläuft, sondern einen einzigen aktualisierten Eintrag mit der
neuesten Information.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
