---
slug: abo-rechnungen
title: Abo-Rechnungen
category: bedienen
order: 130
keywords: [abo, abo-rechnung, wiederkehrend, recurring, vorlage, automatisch, scheduler]
---

# Abo-Rechnungen

Eine Abo-Rechnung ist eine Rechnung, die sich in regelmäßigen
Abständen automatisch wiederholt. Du legst sie einmal als Vorlage
an und sagst Klein.Buch, in welchem Rhythmus daraus neue
Rechnungen werden sollen.

Die Vorlagen erreichst Du über die Rechnungs-Liste: klick im
Hauptmenü auf "Rechnungen" und dort oben rechts auf
**"Wiederkehrende Rechnungen"**.

## Wann eine Abo-Rechnung sinnvoll ist

Typische Fälle: monatliche Wartungs-Pauschalen, jährliche
Lizenz-Gebühren, regelmäßige Beratungs-Stunden zum festen Satz. Wenn
sich Kunde, Positionen und Betrag nicht oder selten ändern, spart
Dir die Abo-Funktion das wiederholte Anlegen jeden Monat.

## Vorlage anlegen

Klick auf **"+ Neue Vorlage"**. Wähle Kunde, Positionen und Preise
wie bei einer normalen Rechnung. Setze den Rhythmus: **monatlich**,
**vierteljährlich**, **halbjährlich** oder **jährlich**. Trage den
nächsten Stichtag ein und optional ein Laufzeit-Ende. Der erste
Stichtag darf nicht in der Vergangenheit liegen; Rechnungen für
zurückliegende Zeiträume legst Du einzeln an. Danach schreibt
Klein.Buch den Stichtag selbst fort.

Wähle den Modus:

**Entwurf.** Klein.Buch legt zur Fälligkeit eine neue
Entwurfs-Rechnung an. Du prüfst sie und stellst sie manuell aus.

**Auto.** Klein.Buch erzeugt die Rechnung und stellt sie sofort
aus, ohne Versand.

**Auto + Versand.** Klein.Buch erzeugt, stellt aus und verschickt
die Rechnung per E-Mail an das Standard-Postfach. Schlägt der
Versand fehl (Mail-Server nicht erreichbar, Passwort falsch),
bleibt die Rechnung ausgestellt liegen und Klein.Buch legt einen
Eintrag unter "Hinweise" an. Du kannst die Rechnung dann manuell
verschicken.

Speichere die Vorlage. Klein.Buch zeigt Dir den nächsten Stichtag.

## Automatischer Lauf

Klein.Buch prüft alle fünf Minuten im Hintergrund, ob eine
Abo-Rechnung fällig ist. Das passiert auf dem Server, auch wenn kein
Browser geöffnet ist, aber nur, solange Klein.Buch entsperrt ist. Nach
einem Neustart des Containers ist Klein.Buch gesperrt, bis Du Dich
wieder anmeldest. Das Öffnen der Vorlagen-Liste löst nichts aus; wer
nicht auf den nächsten Lauf warten will, klickt auf "fällige jetzt
erstellen".

Bricht ein Lauf mitten in einer Rechnung ab (etwa weil der Container
neu startet), setzt der nächste Lauf genau dort fort: Er stellt die
bereits angelegte Rechnung aus und verschickt sie bei Bedarf. Eine
zweite Rechnung für denselben Termin entsteht nicht.

## Verpasste Termine

Wenn Läufe ausgefallen sind (Klein.Buch war gesperrt oder der
Container lief nicht) oder eine Vorlage pausiert war, sind mehrere
Termine offen. Jede nachgeholte Rechnung trägt das **heutige**
Ausstellungsdatum (so verlangt es §14 Abs. 4 Nr. 3 UStG) und den
Leistungszeitraum der ursprünglichen Periode.

- **Bis zu drei offene Termine** holt Klein.Buch selbst nach. Nur der
  jüngste Termin folgt dem Modus der Vorlage (also z. B. ausstellen
  und verschicken). Die älteren entstehen als **Entwurf**: Nachgeholte
  Rechnungen verschickt Klein.Buch nie automatisch. Ein Eintrag unter
  "Hinweise" nennt die Entwürfe; prüfe sie, stelle sie aus und
  verschicke sie selbst.
- **Mehr offene Termine:** Klein.Buch erstellt nichts und meldet
  "Entscheidung nötig". Klick in der Vorlagen-Liste auf "Rückstand …"
  und wähle **Nachholen** (der nächste Lauf erstellt die Termine,
  höchstens 24, ältere als Entwurf) oder **Ab heute fortsetzen** (die
  verpassten Termine werden ausgelassen, weiter geht es mit dem ersten
  Termin ab heute).

Dieselbe Frage kommt, wenn Du eine pausierte Vorlage mit verpassten
Terminen fortsetzt. Eine Bestätigung zum Nachholen gilt für die Termine
bis zu dem Tag, an dem Du sie gibst; kommen danach wieder mehr als drei
Termine zusammen, fragt Klein.Buch erneut.

Klein.Buch überspringt eine Vorlage mit Hinweis, statt eine Rechnung
anzulegen, wenn der Kunde archiviert oder anonymisiert ist oder wenn
eine Position trotz Kleinunternehmer-Regelung Umsatzsteuer ausweist.

## Angehaltene Abo-Läufe

Geht die Uhr des Servers falsch (sie steht vor dem letzten Lauf oder
mehr als 400 Tage danach) oder wurde ein Backup wiederhergestellt,
hält Klein.Buch alle automatischen Abo-Läufe an. Oben erscheint dann
ein Banner. Prüfe Uhrzeit, Rechnungen und Kosten und klick auf
"Prüfen und freigeben"; dafür fragt Klein.Buch Dein Passwort und den
App-Code erneut ab. Nach einer Wiederherstellung gibst Du nur auf der
produktiven Instanz frei, nie auf einer Probe-Instanz (siehe "[Backup und
Wiederherstellen](/help/backup-und-wiederherstellen)"). Termine, die nach
dem Stand des Backups schon abgerechnet wurden, rechnet Klein.Buch nicht
noch einmal ab; kennt es den Stand einer Vorlage nicht, zeigt die Liste
"Rückstand …", und nachgeholte Rechnungen entstehen nur als Entwurf.

## Vorlage anpassen

Du kannst die Vorlage jederzeit ändern: Preise, Positionen,
Rhythmus, Automatik-Modus. Änderungen wirken sich nur auf zukünftig
erzeugte Rechnungen aus. Bereits festgeschriebene Abo-Rechnungen
bleiben unverändert.

Pausen sind möglich: Klick auf "Pausieren". Klein.Buch erzeugt dann
keine neuen Rechnungen mehr, bis Du die Vorlage wieder fortsetzt.
Läuft gerade ein Termin dieser Vorlage, wird er noch fertig; danach
wirkt die Pause.

## Vorlage beenden

Wenn Du eine Abo-Beziehung dauerhaft beendest, trag in der Vorlage
das Laufzeit-Ende auf das Schluss-Datum. Ab dann erzeugt Klein.Buch
keine neuen Rechnungen mehr. Die bisherigen Rechnungen bleiben
unverändert sichtbar.

## Versand-Postfach

Im Modus "Auto + Versand" nutzt Klein.Buch immer das Standard-
Postfach aus den Einstellungen. Es gibt in v1.0 keine Möglichkeit,
pro Vorlage ein abweichendes Postfach festzulegen — wer das
braucht, prüft die Standard-Wahl in "Einstellungen →
E-Mail-Versand".

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.11*
