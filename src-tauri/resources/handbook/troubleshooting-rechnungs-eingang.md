---
slug: troubleshooting-rechnungs-eingang
title: Probleme mit dem Rechnungs-Eingang
category: troubleshooting
order: 560
keywords: [rechnungs-eingang, ordner, fehler, fehlgeschlagen, sync, onedrive, profil, einlesen, drop-folder]
---

# Probleme mit dem Rechnungs-Eingang

Der Rechnungs-Eingang ist der überwachte Ordner für eingehende
elektronische Rechnungen. Wie er funktioniert, steht im Kapitel
"Eingangsrechnungen via Ordner". Dieses Kapitel sammelt die typischen
Stolpersteine.

## Datei landet in `failed/` und nicht in `processed/`

Wenn Klein.Buch eine Datei nicht übernehmen kann, wandert sie nach
`failed/`. Das Original bleibt liegen, damit Du sie ansehen oder beim
Absender reklamieren kannst. Die wichtigsten Gründe:

1. Die Datei ist eine ZUGFeRD-PDF mit einem nicht akzeptierten Profil.
   Klein.Buch akzeptiert nur `EN16931`, `EXTENDED` und `XRECHNUNG`. Die
   alten Profile `MINIMUM` und `BASIC-WL` sind seit 2025 keine
   gültigen E-Rechnungen mehr. Klein.Buch zeigt Dir das erkannte
   Profil im Hinweis-Eintrag. Bitte den Absender um eine richtige
   E-Rechnung. Oder erfasse die Position selbst über "Kosten" und
   "Neue Kosten-Position".
2. Die XRechnung-Datei ist beschädigt (zum Beispiel ein abgebrochener
   Download oder eine Datei, die als XRechnung aussieht, aber kein
   gültiges XML enthält). Öffne die Datei in einem Text-Editor. Wenn
   sie unleserlich oder leer ist, frag beim Absender nach einer neuen
   Version.
3. Die Datei hat eine andere Endung als `.xml` oder `.pdf`. Klein.Buch
   prüft nur diese beiden. Eine ZIP-Datei, eine Word-Datei oder eine
   `.eml`-Mail-Datei wandert direkt nach `failed/`. Entpacke die Datei
   gegebenenfalls und leg die enthaltene Rechnung selbst in den
   Rechnungs-Eingang.
4. Die ZUGFeRD-PDF enthält gar keine eingebettete XRechnung-Datei. Das
   passiert, wenn der Absender Dir eine normale PDF schickt und nur so
   tut, als wäre es ZUGFeRD. Die PDF kannst Du wie einen Papier-Beleg
   behandeln: über "Kosten" und "Neue Kosten-Position" eintragen und
   das PDF dort als Beleg anhängen.

## Mein Ordner wird gar nicht überwacht

Drei Dinge prüfen:

1. Ist der Toggle in "Einstellungen" und "Rechnungs-Eingang" wirklich
   eingeschaltet?
2. Steht in dem Feld der richtige Pfad? Klein.Buch zeigt rechts neben
   dem Feld einen Status: "Ordner ok" heißt, alles passt. Andere
   Meldungen erklären das Problem in einem Satz.
3. Existiert der Ordner überhaupt? Wenn Du ihn nachträglich umbenannt
   oder verschoben hast, kennt Klein.Buch den alten Pfad nicht mehr.
   Wähle den Ordner neu aus und speichere.

Klick danach auf "Jetzt prüfen". Wenn dann ein Hinweis kommt, weißt Du,
dass die Übernahme läuft. Wenn nichts passiert, schau unter
"Hinweise": dort steht der konkrete Fehler.

## Klein.Buch findet die Datei nicht, obwohl sie da liegt

Klein.Buch sieht nur Dateien, die direkt im Rechnungs-Eingang
liegen, nicht in Unter-Ordnern (außer den selbst angelegten
`processed/` und `failed/`, die Klein.Buch zum Sortieren nutzt).

Häufig liegt das Problem aber an einem Cloud-Sync. Wenn Du den
Rechnungs-Eingangs-Ordner in OneDrive, iCloud, Dropbox oder Nextcloud
abgelegt hast, ist die Datei manchmal noch ein Platzhalter und gar nicht
wirklich auf der Festplatte. Klein.Buch sieht den Platzhalter, weiß
aber nicht, dass da echte Daten dahinterstehen.

Lösung: Bei OneDrive klick mit der rechten Maustaste auf den
Rechnungs-Eingangs-Ordner, dann auf "Immer auf diesem Gerät behalten".
Damit lädt OneDrive alle Dateien echt herunter. Bei den anderen
Cloud-Anbietern gibt es eine ähnliche Option (oft "Pin to device" oder
"Stets verfügbar halten").

## Eine Datei taucht doppelt in den Kosten auf

Das passiert in zwei Fällen:

1. Du hast dieselbe XRechnung sowohl manuell über "E-Rechnung
   importieren" hochgeladen und zusätzlich in den Rechnungs-Eingang
   gelegt. Beide Wege erzeugen einen Eingangsbeleg. Lösche keinen
   davon manuell (Belege löschen ist gesetzlich verboten). Erzeuge
   stattdessen einen Storno auf den Beleg, der wegfallen soll.
2. Du hast die Datei aus `processed/` wieder herausgezogen und erneut
   in den Rechnungs-Eingang gelegt. Klein.Buch behandelt sie als neue
   Datei. Vermeide das. Wenn Du eine bereits übernommene Rechnung
   noch einmal ansehen willst, mach das im Kosten-Detail über den
   Button "Roh-XML anzeigen" und nicht durch erneute Übernahme.

## Klein.Buch löscht den Ordner-Inhalt nicht selbst

Das ist Absicht. Klein.Buch verschiebt Dateien nach `processed/` oder
`failed/`, löscht aber nichts. Wenn Du die Unterordner aufräumen willst,
mach das im Datei-Explorer. Klein.Buch braucht die Dateien dort nach
der Übernahme nicht mehr. Der Beleg selbst liegt sicher im internen
Klein.Buch-Archiv und ist 10 Jahre nachweisbar.

## Hinweis kommt nicht

Wenn Klein.Buch eine Rechnung übernimmt, schreibt es einen Eintrag
unter "Hinweise". Bei Erfolg ist der Eintrag stumm (kein Pop-up),
bei Fehler zusätzlich als OS-Benachrichtigung sichtbar — sofern Du
sie in der Regel aktiviert hast. Wenn Du gar keine Hinweise siehst,
geh im Hauptmenü auf "Hinweise" und klick oben rechts auf
"Erinnerungen einstellen". Prüf, ob die zwei Regeln
"Rechnungs-Eingang: Rechnung übernommen" und "Rechnungs-Eingang:
Übernahme fehlgeschlagen" angeschaltet sind. Den Erfolgs-Hinweis
kannst Du dort auch lauter machen.

## Wenn nichts hilft

Lös einen manuellen Lauf über "Jetzt prüfen" aus. Wenn dabei ein
Fehler kommt, kopiere die Meldung und melde den Fall im
Klein.Buch-Repository auf GitHub. Den Link zum Repository findest
Du im "Über"-Dialog (Hauptmenü unten neben "Hilfe"). Hänge die
fehlerhafte XML-Datei oder die Inhalte einer Rechnung nicht an;
sie könnten persönliche Daten enthalten.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
