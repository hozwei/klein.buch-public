---
slug: troubleshooting-sidecar
title: Sidecar-Probleme
category: troubleshooting
order: 510
keywords: [sidecar, mustang, kosit, xrechnung, fehler, start, validierung]
---

# Sidecar-Probleme

Klein.Buch startet im Hintergrund einen Hilfs-Prozess für Mustang
(XRechnung-Erzeugung) und KoSIT (XRechnung-Prüfung). Diesen Prozess
nennen wir Sidecar. Wenn der Sidecar nicht hochfährt, kann
Klein.Buch keine XRechnung erzeugen oder prüfen, und das Erstellen
oder Importieren von E-Rechnungen schlägt fehl.

## Symptome

1. Beim App-Start erscheint die Meldung "Sidecar-Health-Check
   fehlgeschlagen".
2. Beim Festschreiben einer Rechnung kommt ein Fehler "XRechnung
   konnte nicht erzeugt werden".
3. Beim Import einer fremden XRechnung kommt "KoSIT-Validator
   nicht erreichbar".

## Erste Schritte

Beende Klein.Buch komplett (Fenster schließen, nicht nur
minimieren) und starte die App neu. Der Sidecar fährt mit der App
hoch und prüft seinen Health-Endpunkt; nach erfolgreichem Start
verschwindet die Fehlermeldung.

Wenn die Meldung bleibt, schau in die Log-Dateien. Den Daten-Pfad
zeigt Klein.Buch im "Über"-Dialog (Hauptmenü → "Über"). Die
Sidecar-Logs liegen im Unterordner `logs/sidecar/`.

## Häufige Ursachen

1. Anti-Virus-Software blockiert den Sidecar-Start. Manche
   Schutz-Programme erkennen die im Sidecar gebündelte Java-
   Laufzeit fälschlich als verdächtig. Lösung: In den Anti-Virus-
   Einstellungen die Klein.Buch-Installation als Ausnahme
   eintragen. Wenn Du im Unternehmens-Netzwerk arbeitest, frag
   den Administrator.
2. Datei-Berechtigungen sind falsch. Manchmal verliert die
   Sidecar-Datei beim Verschieben von Hand ihre Ausführungs-
   Rechte. Klein.Buch zeigt in diesem Fall den genauen Datei-Pfad.
   Re-installiere Klein.Buch oder setze die Berechtigungen über
   das Eigenschaften-Dialog im Windows-Explorer wieder zurück.
3. Port-Konflikt. Der Sidecar braucht einen lokalen Port. Wenn
   dieser von einer anderen Software belegt ist, schlägt der Start
   fehl. Klein.Buch wählt automatisch einen freien Port und
   protokolliert die Wahl im Log.

## Wenn ein Re-Start nichts bringt

1. Schließe Klein.Buch komplett (nicht nur minimieren).
2. Starte den Rechner neu. Das löst die meisten Port- und
   Berechtigungs-Probleme.
3. Starte Klein.Buch erneut. Der Sidecar fährt mit der App hoch.

Wenn auch das nichts bringt, drück die Tastenkombination
Windows + R, tipp `appwiz.cpl` ein und kontrolliere, ob
Klein.Buch korrekt installiert ist. Eine Reparatur-Installation
über den Installer ist möglich.

## Eingangs-XRechnungen ohne Validierung importieren

Wenn der Sidecar dauerhaft nicht laufen will und Du dringend eine
fremde XRechnung importieren musst, erlaubt Klein.Buch das auch
ohne KoSIT-Prüfung. Klick beim Import auf "Trotzdem importieren".
Die App liest die XML-Daten und speichert sie; die formelle
Prüfung entfällt einmalig.

Bei Ausgangs-Rechnungen geht das nicht. Eine neue Rechnung
erzeugen verlangt einen funktionierenden Sidecar.

## Fehler melden

Wenn der Sidecar dauerhaft nicht startet, schau Dir die letzten
Zeilen der Log-Datei an und melde das Problem im Klein.Buch-
Repository. Hänge die Log-Auszüge an. Persönliche Daten kommen in
den Sidecar-Logs nicht vor.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
