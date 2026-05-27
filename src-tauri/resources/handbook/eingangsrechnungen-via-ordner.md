---
slug: eingangsrechnungen-via-ordner
title: Eingangsrechnungen via Ordner
category: bedienen
order: 145
keywords: [rechnungs-eingang, ordner, automatisch, eingangsrechnung, einlesen, importieren, xrechnung, zugferd, sync, drop-folder]
---

# Eingangsrechnungen via Ordner

Wenn Du oft elektronische Rechnungen per E-Mail bekommst, ist das
Stück-für-Stück-Hochladen über "Kosten" und "E-Rechnung importieren" auf
Dauer lästig. Klein.Buch kann stattdessen einen Ordner auf Deiner
Festplatte automatisch überwachen. Sobald eine neue Datei darin liegt,
liest Klein.Buch sie ein, legt einen Eingangsbeleg an und schiebt die
Original-Datei in einen Unterordner für Erledigtes.

In Klein.Buch heißt dieser überwachte Ordner "Rechnungs-Eingang" (so
findest Du ihn auch im Menü Einstellungen).

## Wozu das gut ist

Ein typischer Fall: Du bekommst von Deinem Software-Anbieter jeden Monat
eine ZUGFeRD-Rechnung als PDF-Anhang per E-Mail. Bisher musstest Du die
PDF aus der Mail ziehen, Klein.Buch öffnen, in der Kosten-Liste auf
"E-Rechnung importieren" klicken und die Datei wählen. Mit dem
Rechnungs-Eingang reicht: PDF aus der Mail in den Ordner kopieren,
fertig. Beim nächsten Klein.Buch-Start (oder im Hintergrund alle fünf
Minuten) ist der Beleg drin.

Wenn Du einen Cloud-Ordner wie OneDrive nutzt, kannst Du auch dort einen
Unterordner als Rechnungs-Eingang festlegen. Du oder Dein
Steuerberater können dann Rechnungen vom Handy oder Tablet aus in den
Ordner werfen, und Klein.Buch holt sie sich am Hauptrechner.

## Einrichten

Klick im Hauptmenü auf "Einstellungen" und dort auf "Rechnungs-Eingang".

Drei Schritte:

1. Wähle einen Ordner, in den Du Deine Eingangsrechnungen ablegen
   willst. Idealerweise ein eigener Ordner, in dem nichts anderes
   liegt. Beispiel: `C:\Klein.Buch\Rechnungs-Eingang`.
2. Schalte den Toggle "Rechnungs-Eingang aktivieren" ein.
3. Klick auf "Speichern".

Klein.Buch prüft sofort, ob der Pfad existiert und ob die App dort
schreiben darf. Stimmt etwas nicht, kommt eine klare Fehlermeldung.

## Was passiert mit einer abgelegten Datei

Jede Datei wird einzeln angesehen.

Bei einer XRechnung-Datei (Dateiendung `.xml`) oder einer ZUGFeRD-PDF
(Dateiendung `.pdf` mit eingebetteter XML-Datei) liest Klein.Buch die
Daten aus. Daraus entsteht ein neuer Eingangsbeleg in der Kosten-Liste.
Die Original-Datei wandert anschließend in den Unterordner
`processed/JJJJ-MM/`. JJJJ ist das aktuelle Jahr, MM der Monat. So
bleibt der Rechnungs-Eingang leer und Du siehst auf einen Blick, was
schon übernommen ist.

Bei einer Datei mit anderer Endung (zum Beispiel `.zip` oder `.docx`)
oder bei einer kaputten XML wandert die Datei nach `failed/`. Sie wird
nicht gelöscht. Klein.Buch legt unter "Hinweise" einen Eintrag an
und zeigt zusätzlich eine kurze Meldung am Bildschirm. Du kannst die Datei dann
in Ruhe ansehen, beim Absender nachfragen oder die Position notfalls
selbst in Kosten eintragen.

Versteckte System-Dateien (zum Beispiel `Thumbs.db` unter Windows oder
`.DS_Store` auf einem Mac, oder halbfertige `*.tmp`-Dateien während
eines Cloud-Sync) lässt Klein.Buch unberührt. Sie wandern nicht nach
`failed/` und tauchen nicht unter "Hinweise" auf.

## Wann Klein.Buch prüft

An drei Stellen:

1. Direkt beim Klein.Buch-Start. Wenn Du am Morgen die App öffnest, sind
   alle Dateien, die über Nacht im Ordner gelandet sind, sofort drin.
2. Im Hintergrund alle fünf Minuten, während Klein.Buch läuft.
3. Auf Knopfdruck. Auf der Rechnungs-Eingang-Seite gibt es einen Button
   "Jetzt prüfen". Damit löst Du den Lauf manuell aus, ohne fünf
   Minuten warten zu müssen.

## Was Du im Eingangsbeleg noch tun musst

Klein.Buch füllt aus der E-Rechnung Lieferant, Beträge, Beleg-Datum und
Beschreibung automatisch aus. Was Du noch ergänzen solltest:

1. Das Zahlungs-Datum. Klein.Buch weiß nicht, wann Du bezahlt hast.
   Solange Du es nicht einträgst, taucht die Position auch nicht in der
   EÜR (Einnahmen-Überschuss-Rechnung, die einfache Steuer-Aufstellung)
   auf. Du findest den Eingangsbeleg in der Kosten-Liste mit einem
   "ohne Zahlung"-Hinweis.
2. Die Kategorie. Klein.Buch nimmt zunächst "Sonstiges". Wenn Du die
   passende Kategorie kennst (zum Beispiel "Software" oder
   "Reisekosten"), ändere sie kurz und speichere.

## Welche Profile akzeptiert werden

Bei ZUGFeRD gibt es verschiedene Profile (also Varianten des Formats).
Klein.Buch akzeptiert die Profile, die seit 2025 das Gesetz als gültige
E-Rechnung verlangt: `EN16931`, `EXTENDED` und `XRECHNUNG`.

Die alten Profile `MINIMUM` und `BASIC-WL` lehnt Klein.Buch ab. Beide
sind seit der E-Rechnungs-Pflicht 2025 keine gültigen Rechnungen mehr,
sondern nur noch maschinenlesbare Beilagen zu einer Papier- oder
PDF-Rechnung. Wenn Du so eine Datei im Rechnungs-Eingang ablegst,
wandert sie nach `failed/` mit einer eindeutigen Meldung welches Profil
erkannt wurde. Frag dann beim Absender nach einer richtigen
E-Rechnung. Wenn das nicht klappt, trägst Du die Position selbst ein.

## Was der Rechnungs-Eingang nicht tut

Klein.Buch durchsucht nur Dateien, die direkt im Rechnungs-Eingang
liegen. Unterordner werden nicht geprüft (außer den selbst angelegten
`processed/` und `failed/`). Wenn Du Unterordner-Struktur für Dein
Mail-Archiv brauchst, baust Du Dir die selbst und ziehst die Dateien
einzeln in den Rechnungs-Eingang rein.

Klein.Buch löscht im Rechnungs-Eingang nichts ungefragt. Dateien
wandern nur nach `processed/` oder `failed/`, beides bleibt liegen.
Wenn `failed/` oder ältere `processed/JJJJ-MM/`-Unterordner sich
füllen, kannst Du sie gefahrlos im Datei-Explorer aufräumen.
Klein.Buch braucht sie für die Buchhaltung nicht mehr, der Beleg
selbst liegt sicher im internen Klein.Buch-Archiv.

## Wenn etwas nicht funktioniert

Schau im Kapitel "Probleme mit dem Rechnungs-Eingang" unter
"Troubleshooting" nach. Dort stehen die typischen Fälle: Datei landet
in `failed/`, Klein.Buch findet die Datei nicht, der Cloud-Sync hinkt
hinterher.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
