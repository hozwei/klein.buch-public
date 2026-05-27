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
nächsten Stichtag ein und optional ein Laufzeit-Ende.

Wähle den Modus:

**Entwurf.** Klein.Buch legt zur Fälligkeit eine neue
Entwurfs-Rechnung an. Du prüfst sie und stellst sie manuell aus.

**Auto.** Klein.Buch erzeugt die Rechnung und stellt sie sofort
aus, ohne Versand.

**Auto + Versand.** Klein.Buch erzeugt, stellt aus und verschickt
die Rechnung per E-Mail an das Standard-Postfach. Schlägt der
Versand fehl (Server nicht erreichbar, OAuth-Token abgelaufen),
bleibt die Rechnung ausgestellt liegen und Klein.Buch legt einen
Eintrag unter "Hinweise" an. Du kannst die Rechnung dann manuell
verschicken.

Speichere die Vorlage. Klein.Buch zeigt Dir den nächsten Stichtag.

## Automatischer Lauf

Klein.Buch prüft alle fünf Minuten im Hintergrund, ob eine
Abo-Rechnung fällig ist (das passiert nur, solange die App geöffnet
und entsperrt ist). Außerdem prüft die App beim Öffnen der Vorlagen-
Liste sofort, ob etwas nachzuholen ist.

Wenn ein Lauf zwischendurch ausgefallen ist (App war geschlossen,
Rechner aus), holt Klein.Buch beim nächsten Start die ausgelassenen
Termine nach. Klein.Buch erzeugt pro verpasster Periode eine eigene
Rechnung — alle mit dem **heutigen** Ausstellungsdatum (so verlangt
es §14 Abs. 4 Nr. 3 UStG), aber jeweils mit dem korrekten
Leistungszeitraum der ursprünglichen Periode.

## Vorlage anpassen

Du kannst die Vorlage jederzeit ändern: Preise, Positionen,
Rhythmus, Automatik-Modus. Änderungen wirken sich nur auf zukünftig
erzeugte Rechnungen aus. Bereits festgeschriebene Abo-Rechnungen
bleiben unverändert.

Pausen sind möglich: Klick auf "Vorlage pausieren". Klein.Buch
erzeugt dann keine neuen Rechnungen mehr, bis Du die Vorlage wieder
aktivierst. Eine pausierte Vorlage holt keine Termine nach.

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

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
