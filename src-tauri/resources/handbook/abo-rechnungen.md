---
slug: abo-rechnungen
title: Abo-Rechnungen
category: bedienen
order: 130
keywords: [abo, abo-rechnung, abo-vorlage, wiederkehrend, recurring, vorlage, automatisch, scheduler, pausieren]
---

# Abo-Rechnungen

Eine Abo-Rechnung ist eine Rechnung, die sich in regelmäßigen
Abständen automatisch wiederholt. Du legst sie einmal als
Abo-Vorlage an und sagst Klein.Buch, in welchem Rhythmus daraus neue
Rechnungen werden sollen.

Die Abo-Vorlagen erreichst Du über die Rechnungs-Liste: klick im
Hauptmenü auf "Rechnungen" und dort oben auf **"Abo-Rechnungen"**
(auf dem Handy im Menü "Mehr"). Die Seite "Abo-Rechnungen" listet
Deine Abo-Vorlagen mit Kunde, Rhythmus, nächster Fälligkeit und
Modus; pausierte blendest Du mit "Pausierte anzeigen" ein.

## Wann eine Abo-Rechnung sinnvoll ist

Typische Fälle: monatliche Wartungs-Pauschalen, jährliche
Lizenz-Gebühren, regelmäßige Beratungs-Stunden zum festen Satz. Wenn
sich Kunde, Positionen und Betrag nicht oder selten ändern, spart
Dir die Abo-Funktion das wiederholte Anlegen jeden Monat.

## Abo-Vorlage anlegen

Klick auf **"+ Neue Abo-Vorlage"**. Gib ihr eine "Bezeichnung
(intern)" und wähle Kunde, Positionen und Preise wie bei einer
normalen Rechnung. Setze unter "Frequenz" den Rhythmus:
**monatlich**, **vierteljährlich**, **halbjährlich** oder
**jährlich**, dazu den "Stichtag (Tag im Monat, 1–31)" und das
"Zahlungsziel" in Tagen. Trage die "Nächste Fälligkeit" ein und
optional ein "Laufzeit-Ende". Schreibst Du an eine Behörde, trägst
Du die "Leitweg-ID (optional)" ein; jede Rechnung aus der Vorlage
bekommt sie. Die erste Fälligkeit darf nicht in der
Vergangenheit liegen; Rechnungen für zurückliegende Zeiträume legst
Du einzeln an. Danach schreibt Klein.Buch die Fälligkeit selbst fort;
beim Bearbeiten lässt sie sich nicht mehr ändern.

Wähle unter "Was soll am Stichtag passieren?" den Modus:

**Nur Entwurf.** Klein.Buch legt zur Fälligkeit eine neue
Entwurfs-Rechnung an und meldet sie. Du prüfst sie und stellst sie
selbst aus.

**Automatisch ausstellen.** Klein.Buch erzeugt die Rechnung und
stellt sie sofort aus, ohne Versand; Du versendest selbst.

**Ausstellen + senden.** Klein.Buch erzeugt, stellt aus und verschickt
die Rechnung per E-Mail an den Kunden, über das Standard-Postfach.
Schlägt der Versand fehl (Mail-Server nicht erreichbar, Passwort
falsch), bleibt die Rechnung ausgestellt liegen und Klein.Buch legt
einen Eintrag unter "Hinweise" an. Du kannst die Rechnung dann
manuell verschicken.

Jede erzeugte Rechnung trägt den Abrechnungszeitraum als
Leistungszeitraum: vom Stichtag bis zum Tag vor dem nächsten Stichtag,
bei einem quartalsweisen Abo ab 01.04.2026 also 01.04.2026 – 30.06.2026
(im PDF und als Rechnungszeitraum in der E-Rechnung). Das Formular
zeigt unter dem Schalter, welchen Zeitraum die nächste Rechnung
trägt. Mit dem Schalter "Leistungszeitraum zusätzlich in Position 1
nennen" steht er auch im Text der ersten Position, zum Beispiel
"Leistungszeitraum: Mai 2026" bei einem ganzen Monat; ohne Schalter
fehlt nur dieser Text, der Zeitraum selbst bleibt auf der Rechnung.
Eine Leitweg-ID der Vorlage bekommt jede erzeugte Rechnung.

Für den Steuerstatus (§19 oder Regelbesteuerung) zählt bei
Abo-Rechnungen das Ende des Abrechnungszeitraums. Der Hinweis über dem
Formular nennt deshalb den Status am Ende des nächsten
Abrechnungszeitraums.

Klick auf "Abo-Vorlage anlegen". Die Liste zeigt danach die nächste
Fälligkeit.

## Automatischer Lauf

Klein.Buch prüft alle fünf Minuten im Hintergrund, ob eine
Abo-Rechnung fällig ist. Das passiert auf dem Server, auch wenn kein
Browser geöffnet ist, aber nur, solange Klein.Buch entsperrt ist. Nach
einem Neustart des Containers ist Klein.Buch gesperrt, bis Du Dich
wieder anmeldest. Das Öffnen der Liste löst nichts aus; wer
nicht auf den nächsten Lauf warten will, klickt auf "N fällige jetzt
erstellen" (der Knopf erscheint, sobald etwas fällig ist, N ist die
Anzahl) oder bei einer einzelnen Abo-Vorlage auf "Jetzt erstellen".

Stellt dabei eine Abo-Vorlage die Rechnung aus oder versendet sie
("Automatisch ausstellen" oder "Ausstellen + senden"), fragt
Klein.Buch vorher nach und nennt die Folgen, beim Versand auch den
Empfänger. Bei "Nur Entwurf" entsteht der Entwurf ohne Rückfrage. Die
Meldung danach sagt, was tatsächlich entstanden ist: ein Entwurf,
eine ausgestellte oder eine ausgestellte und gesendete Rechnung.
"Jetzt erstellen" öffnet anschließend die neue Rechnung.

Bricht ein Lauf mitten in einer Rechnung ab (etwa weil der Container
neu startet), setzt der nächste Lauf genau dort fort: Er stellt die
bereits angelegte Rechnung aus und verschickt sie bei Bedarf. Eine
zweite Rechnung für denselben Termin entsteht nicht.

## Verpasste Termine

Wenn Läufe ausgefallen sind (Klein.Buch war gesperrt oder der
Container lief nicht) oder eine Abo-Vorlage pausiert war, sind mehrere
Termine offen. Jede nachgeholte Rechnung trägt das **heutige**
Ausstellungsdatum (so verlangt es §14 Abs. 4 Nr. 3 UStG) und den
Leistungszeitraum der ursprünglichen Periode.

- **Bis zu drei offene Termine** holt Klein.Buch selbst nach. Nur der
  jüngste Termin folgt dem Modus der Abo-Vorlage (also z. B. ausstellen
  und verschicken). Die älteren entstehen als **Entwurf**: Nachgeholte
  Rechnungen verschickt Klein.Buch nie automatisch. Ein Eintrag unter
  "Hinweise" nennt die Entwürfe; prüfe sie, stelle sie aus und
  verschicke sie selbst.
- **Mehr offene Termine:** Klein.Buch erstellt nichts und meldet
  "Entscheidung nötig". Klick in der Liste bei der Abo-Vorlage auf
  "Rückstand …" und wähle **Nachholen** (der nächste Lauf erstellt die
  Termine, höchstens 24, ältere als Entwurf) oder **Ab heute
  fortsetzen** (die verpassten Termine werden ausgelassen, weiter geht
  es mit dem ersten Termin ab heute).

Dieselbe Frage kommt, wenn Du eine pausierte Abo-Vorlage mit verpassten
Terminen fortsetzt. Eine Bestätigung zum Nachholen gilt für die Termine
bis zu dem Tag, an dem Du sie gibst; kommen danach wieder mehr als drei
Termine zusammen, fragt Klein.Buch erneut.

Klein.Buch überspringt eine Abo-Vorlage mit Hinweis, statt eine Rechnung
anzulegen, wenn der Kunde archiviert oder anonymisiert ist oder wenn
die Positionen nicht zum Steuerstatus am Termin passen: eine Position
mit Umsatzsteuer, obwohl am Termin die Kleinunternehmer-Regelung gilt,
oder eine steuerbefreite Position (Steuerart E), obwohl dort die
Regelbesteuerung gilt. Maßgeblich ist der Status am Termin, nicht der
von heute; das Formular zeigt den Status am ersten bzw. nächsten Termin.
Trägst Du einen Wechsel ein, nennt Klein.Buch die betroffenen
Abo-Vorlagen (Kapitel "Kleinunternehmer-Regelung").

## Angehaltene Abo-Läufe

Geht die Uhr des Servers falsch (sie steht vor dem letzten Lauf oder
mehr als 400 Tage danach) oder wurde ein Backup wiederhergestellt,
hält Klein.Buch alle automatischen Abo-Läufe an. Oben erscheint dann
ein Banner. Prüfe Uhrzeit, Rechnungen und Kosten und klick auf
"Prüfen und freigeben"; dafür fragt Klein.Buch Dein Daten-Passwort und
den App-Code erneut ab. Nach einer Wiederherstellung gibst Du nur auf der
produktiven Instanz frei, nie auf einer Probe-Instanz (siehe "[Backup und
Wiederherstellen](/help/backup-und-wiederherstellen)"). Termine, die nach
dem Stand des Backups schon abgerechnet wurden, rechnet Klein.Buch nicht
noch einmal ab; kennt es den Stand einer Abo-Vorlage nicht, zeigt die
Liste "Rückstand …", und nachgeholte Rechnungen entstehen nur als
Entwurf.

## Abo-Vorlage anpassen

In der Liste steht je Abo-Vorlage eine Aktion, die zu ihrem Zustand
passt: "Rückstand …" bei verpassten Terminen, "Jetzt erstellen" bei
Fälligkeit, "Fortsetzen" bei einer pausierten Abo-Vorlage, sonst
"Bearbeiten". Die übrigen Aktionen stehen im Menü "Mehr" der Zeile.

Über "Bearbeiten" änderst Du die Abo-Vorlage jederzeit: Preise,
Positionen, Rhythmus, Modus. "Änderungen speichern" übernimmt sie.
Änderungen wirken sich nur auf zukünftig erzeugte Rechnungen aus.
Bereits festgeschriebene Abo-Rechnungen bleiben unverändert.

Pausen sind möglich: Wähl im Menü "Mehr" der Zeile "Pausieren".
Klein.Buch fragt nicht nach, weil sich das umkehren lässt; die
Meldung danach bietet "Rückgängig". Klein.Buch erzeugt dann keine
neuen Rechnungen mehr, bis Du die Abo-Vorlage mit "Fortsetzen" wieder
aufnimmst. Läuft gerade ein Termin dieser Abo-Vorlage, wird er noch
fertig; danach wirkt die Pause.

## Abo-Vorlage beenden

Wenn Du eine Abo-Beziehung dauerhaft beendest, trag in der Abo-Vorlage
das Laufzeit-Ende auf das Schluss-Datum. Ab dann erzeugt Klein.Buch
keine neuen Rechnungen mehr; liegt die nächste Fälligkeit nach dem
Laufzeit-Ende, steht die Abo-Vorlage in der Liste als "Beendet". Die
bisherigen Rechnungen bleiben unverändert sichtbar.

## Versand-Postfach

Im Modus "Ausstellen + senden" nutzt Klein.Buch immer das Standard-
Postfach aus den Einstellungen. Es gibt keine Möglichkeit,
pro Abo-Vorlage ein abweichendes Postfach festzulegen — wer das
braucht, prüft die Standard-Wahl in "Einstellungen →
E-Mail-Versand".

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
