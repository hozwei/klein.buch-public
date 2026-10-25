---
slug: hinweise
title: Hinweise
category: bedienen
order: 210
keywords: [hinweise, inbox, benachrichtigung, erinnerung, regel, abhaken, backup]
---

# Hinweise

Klein.Buch sammelt seine Mitteilungen in der Hinweis-Liste (in älteren
Texten auch "Inbox" genannt). Dort siehst Du, was Klein.Buch im
Hintergrund für Dich bemerkt hat: Erinnerungen an wiederkehrende
Aufgaben wie das Erfassen der Belege oder den Abschluss des
Geschäftsjahres, überfällige Rechnungen, Abo-Rechnungen, die
festgeschrieben wurden, Probleme mit dem Backup und Schäden im
Beleg-Archiv.

Hinweise erscheinen nur in Klein.Buch selbst. Klein.Buch schickt Dir
keine E-Mail und keine Mitteilung aufs Telefon. Schau deshalb ab und zu
in die Liste, besonders nach einem Neustart des Servers.

## Liste öffnen

Klick im Hauptmenü auf "Hinweise". Du siehst die offenen Hinweise, den
neuesten oben. Jeder Eintrag hat eine Einstufung ("Dringend", "Achtung"
oder "Info"), einen Titel, Datum und Uhrzeit und einen kurzen Text. Bei
vielen Einträgen führt der Knopf "Öffnen" direkt zur passenden Seite,
zum Beispiel zum Backup-Protokoll.

Hast Du etwas erledigt, klickst Du beim Eintrag auf "Abhaken". Mit "Alle
abhaken" oben räumst Du die ganze Liste auf einmal ab. Abgehakte
Hinweise verschwinden nicht, sie werden nur ausgeblendet. Setzt Du oben
das Häkchen bei "Abgehakte zeigen", siehst Du sie wieder und kannst
nachvollziehen, was wann passiert ist.

Klein.Buch prüft seine Erinnerungen regelmäßig von selbst, solange es
entsperrt ist. Mit "Jetzt prüfen" stößt Du die Prüfung sofort an.

## Erinnerungen einstellen

Welche Erinnerungen Klein.Buch Dir zeigt, legst Du nicht in den
Einstellungen fest, sondern direkt bei den Hinweisen: Ganz unten auf
der Seite "Hinweise" führt "Erinnerungen einstellen" zur Liste der
Regeln. Jede Regel hat einen Schalter "An" oder "Aus" und einen Satz,
der erklärt, was sie tut. Weitere Einstellungen wie eigene Fristen gibt
es nicht. Die Regeln sind:

1. "Monats-Doku-Check": monatliche Erinnerung, die Belege und Kosten des
   Vormonats zu erfassen
2. "Geschäftsjahr abschließen fällig": Erinnerung ab dem 1. Juni, das
   Vorjahr abzuschließen
3. "Backup überfällig": Hinweis, wenn seit mehr als sieben Tagen kein
   Backup auf einem externen Ziel geklappt hat (ohne externes Ziel: wenn
   seit sieben Tagen gar kein Backup mehr entstanden ist)
4. "Rechnung überfällig": Hinweis auf festgeschriebene Rechnungen, deren
   Fälligkeitsdatum überschritten ist
5. "Archiv-Integrität gestört": dringender Hinweis, wenn die monatliche
   Archiv-Prüfung einen Schaden findet
6. "Backup-Ergebnis": Hinweis nach einem erfolgreichen manuellen Backup
   und wenn die lokale Kopie eines Backups nicht geschrieben werden
   konnte
7. "Kosten-Abo fällig": Hinweis am Stichtag eines Kosten-Abos im Modus
   "Erinnerung" (Kapitel "Kosten")

## Hinweise, die sich nicht abschalten lassen

Einige Hinweise betreffen die Sicherheit Deiner Daten so direkt, dass
Klein.Buch sie immer zeigt:

1. "Kein externes Backup-Ziel": einmal pro Woche, solange kein externes
   Backup-Ziel eingerichtet ist
2. "Externe Backup-Spiegelung fehlgeschlagen": wenn eine Kopie auf ein
   externes Ziel nicht geklappt hat
3. "Archiv-Dateien nicht lesbar": wenn die Archiv-Prüfung Dateien nicht
   lesen konnte, meist wegen falscher Dateirechte auf dem Server
4. "PDF-Vorlagen nach dem Restore fehlen": wenn nach einer
   Wiederherstellung Vorlagen fehlen, die Deine Daten benutzen

Dazu kommen Mitteilungen zu Abo-Rechnungen (festgeschrieben, verschickt
oder Versand fehlgeschlagen), zur jährlichen Abschreibung und zu neuen
Fassungen der Vorlagen nach einem Update. Hast Du unter "Einstellungen"
und "Sicherheit & Anmeldung" die Versionsprüfung eingeschaltet, meldet
Klein.Buch hier auch, wenn eine neue Version erschienen ist.

## Doppelte Hinweise

Klein.Buch zeigt denselben Hinweis nicht immer wieder neu. Tritt dieselbe
Bedingung mehrfach auf, entsteht höchstens ein Eintrag pro Tag, bei
manchen Hinweisen pro Woche oder Monat. Scheitert zum Beispiel die Kopie
auf ein externes Ziel bei jedem Backup eines Tages, siehst Du dafür nur
einen Hinweis. Ist das Problem am nächsten Tag noch da, kommt ein neuer.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
