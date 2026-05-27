---
slug: gobd-im-alltag
title: GoBD im Alltag
category: recht-und-steuern
order: 320
keywords: [gobd, unveränderbarkeit, archiv, audit, prüfung, festschreibung, storno]
---

# GoBD im Alltag

GoBD steht für "Grundsätze zur ordnungsmäßigen Führung und
Aufbewahrung von Büchern, Aufzeichnungen und Unterlagen in
elektronischer Form sowie zum Datenzugriff". Es sind die Regeln,
nach denen das Finanzamt Buchhaltungs-Software akzeptiert.

Du musst die GoBD nicht auswendig kennen. Klein.Buch erzwingt die
relevanten Punkte im Code. Es ist trotzdem nützlich, die
Grundgedanken zu verstehen, damit Du weißt, warum die App manche
Dinge nicht erlaubt.

## Unveränderbarkeit

Sobald Du einen Beleg festschreibst, darf er sich nicht mehr
ändern. Klein.Buch sperrt die Kernfelder. Eine Korrektur läuft
ausschließlich über einen Storno-Beleg, der den Original-Beleg
neutralisiert.

Auch das Beleg-Archiv ist write-once. Klein.Buch schreibt das PDF
und das XRechnung-XML beim Festschreiben einmal in den
Archiv-Ordner und vergibt der Datei nur noch Leserechte. Eine
nachträgliche Änderung würde Klein.Buch beim nächsten Zugriff am
SHA-256-Prüfwert erkennen.

## Nachvollziehbarkeit

Jede relevante Aktion landet in einem Audit-Log. Welche Rechnung
wurde wann festgeschrieben, welche Zahlung wurde wann erfasst,
welche Anlage wurde wann ausgebucht. Du kannst das Audit-Log nicht
bearbeiten und nicht löschen. Klein.Buch zeigt es Dir in den
Einstellungen unter "Audit-Log".

## Aufbewahrung 10 Jahre

Rechnungen und alle steuerlich relevanten Belege musst Du 10 Jahre
aufbewahren, gerechnet ab Ende des Kalenderjahres, in dem der
Beleg entstanden ist (§147 Abs. 4 AO). Eine Rechnung mit
Rechnungsdatum 2026 ist also bis zum 31.12.2036 aufzuheben.

Klein.Buch hat kein "Beleg löschen"-Werkzeug. Das ist Absicht.
Die einzige Funktion, die Daten überhaupt löscht, ist der
Factory Reset, und der verlangt vorher einen Export oder eine
Aufbewahrungs-Quittung.

## Vollständigkeit

Alle Geschäftsvorfälle müssen erfasst sein. Eine ausgestellte
Rechnung darf nicht aus der EÜR fehlen. Eine Privatentnahme muss
als solche gebucht sein, nicht als verschwundenes Geld. Das ist
Deine Pflicht; Klein.Buch hilft Dir nur dabei, sie nicht zu
vergessen.

## Zeitnahe Buchung

Geschäftsvorfälle sollen zeitnah erfasst werden. Das Gesetz lässt
einige Wochen Spielraum, aber drei Monate zu warten ist nicht in
Ordnung. Klein.Buch erinnert Dich an offene Vorgänge über die
Hinweise.

## Datenzugriff bei Prüfung

Bei einer Betriebsprüfung muss das Finanzamt Zugriff auf Deine
Daten bekommen, in einem auswertbaren Format. Klein.Buch deckt das
über den Steuerberater-Export ab: ZIP mit PDFs, XRechnung-XMLs,
DATEV-Stapel und Inhalts-Übersicht.

## Was Klein.Buch für Dich macht

1. Sperrt Beleg-Felder nach Festschreibung.
2. Erzwingt Storno statt Löschung.
3. Hasht jedes Archiv-File und prüft den Prüfwert bei jedem
   Zugriff.
4. Führt ein Audit-Log, das nicht editierbar ist.
5. Schreibt zeitnahe Sicherungs-Kopien (Backups).
6. Erzeugt prüfungssichere Exporte für den Steuerberater.

Damit erfüllst Du im Alltag die GoBD-Pflichten, ohne dass Du an
einzelne Punkte denken musst.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
