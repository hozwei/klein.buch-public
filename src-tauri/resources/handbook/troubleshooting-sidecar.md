---
slug: troubleshooting-sidecar
title: Sidecar-Probleme
category: troubleshooting
order: 510
keywords: [sidecar, mustang, kosit, java, xrechnung, fehler, validierung, container-log]
---

# Sidecar-Probleme

Für zwei Aufgaben nutzt Klein.Buch ein Hilfsprogramm in Java: Mustang
erzeugt die ZUGFeRD-PDF mit eingebetteter XRechnung und holt die XML
aus empfangenen ZUGFeRD-PDFs heraus. Der KoSIT-Validator prüft
XRechnungen nach den amtlichen Regeln. Dieses Hilfsprogramm nennen
wir Sidecar. Es steckt im Container von Klein.Buch, die Java-Laufzeit
gehört zum Image. Auf dem Server musst Du kein Java installieren.
Klein.Buch startet den Sidecar bei jedem Vorgang neu und beendet ihn
danach wieder; höchstens zwei Java-Vorgänge laufen gleichzeitig.

## Woran Du das Problem erkennst

1. Beim Ausstellen einer Rechnung erscheint die Meldung "Der Prüf-
   und Konvertierungsdienst (Java) ist nicht verfügbar. Bitte später
   erneut versuchen." mit einer Kennung in Klammern, zum Beispiel
   "(Ref 3f2a…)". Die Rechnung bleibt dann ein Entwurf, es ist nichts
   kaputtgegangen. Angebote brauchen den Sidecar nicht, sie sind keine
   E-Rechnungen.
2. Beim Einlesen eines ZUGFeRD-PDFs über "E-Rechnung importieren"
   kommt dieselbe Meldung.
3. Beim Einlesen einer XRechnung-XML steht auf der Prüfseite "Die
   formale Prüfung (KoSIT) war nicht möglich".

## Erste Schritte

Versuch es nach einer Minute noch einmal. Laufen gerade andere
Java-Vorgänge, zum Beispiel mehrere Abo-Rechnungen auf einmal, wartet
Klein.Buch auf einen freien Platz. Bleibt die Meldung, schau ins
Container-Log. Das ist die Text-Ausgabe, die Klein.Buch im Container
schreibt. Du liest sie als Betreiber auf dem Server, zum Beispiel mit
`docker compose logs klein-buch` oder in der Oberfläche Deiner
Container-Verwaltung. Wie Du das Log liest und filterst, steht im
Kapitel "Fehlersuche im Container".

Such im Log nach der Kennung aus der Meldung. Die passende Zeile
enthält `reference=` mit derselben Kennung; hinter "Sidecar:" steht
in derselben Zeile der eigentliche Grund. Kurz davor stehen Zeilen wie
"KoSIT-Validator startet" oder "Mustang-Sidecar startet" und, wenn
der Vorgang zu Ende kam, "fertig (exit …)".

## Häufige Ursachen

1. Zu wenig Arbeitsspeicher. Jeder Java-Vorgang darf bis zu 512 MB
   belegen. Ist das Speicherlimit des Containers (in der Compose-Datei
   `mem_limit`) zu knapp, beendet der Container den Java-Prozess. Im
   Log steht dann hinter "exit-code" ein Fehlercode oder ein Signal
   wie "signal: 9". Halte Dich an die Werte
   aus dem Compose-Beispiel. Mehr dazu im Kapitel "Speicher und
   Ressourcen".
2. Zeitüberschreitung. Antwortet Java nicht innerhalb von 120
   Sekunden, bricht Klein.Buch den Vorgang ab. Im Log steht dann "hat
   nach 120s nicht geantwortet". Meist ist der Server gerade stark
   ausgelastet oder hat zu wenig Speicher.
3. Kein Platz für Zwischendateien. Der Sidecar schreibt seine
   Zwischendateien unter `/data/tmp` im Daten-Volume. Ist das Volume
   voll oder nicht beschreibbar, scheitert der Vorgang. Prüfe freien
   Platz und Rechte des Volumes (Kapitel "Volumes und Rechte").
4. Zu enge Grenzen in der Compose-Datei. Setzt Du `pids_limit` oder
   `mem_limit` deutlich enger als im Beispiel, kann Java nicht
   starten.
5. "Launcher nicht gefunden" im Log. Dann fehlt das Startskript des
   Sidecars im Image. Das passiert nur bei einem veränderten oder
   beschädigten Image. Zieh das offizielle Image mit demselben
   Versions-Tag neu.

## Container neu starten

Hilft das alles nicht, starte den Container neu, zum Beispiel mit
`docker compose restart klein-buch`. Danach ist Klein.Buch gesperrt,
bis Du Dich im Browser wieder mit Daten-Passwort und App-Code
anmeldest. Erst dann laufen auch fällige Hintergrund-Aufgaben wie das
Tages-Backup weiter.

## Startet der Container gar nicht

Steht im Log die Zeile
`KLEIN_BUCH_SIDECAR_MOCK ist in Release-Builds nicht erlaubt — Start abgebrochen.`,
ist in der Compose-Datei die Variable
`KLEIN_BUCH_SIDECAR_MOCK` gesetzt. Sie ist nur für Tests gedacht und
täuscht eine Prüfung vor, die nicht stattfindet. Entferne die Zeile
und starte den Container neu.

## Eingangsrechnungen ohne Prüfung erfassen

Fällt nur die KoSIT-Prüfung aus, kannst Du eine fremde XRechnung-XML
trotzdem übernehmen. Klein.Buch zeigt den Hinweis "Die formale
Prüfung (KoSIT) war nicht möglich" und lässt Dich die Rechnung als
Kosten festschreiben. Ein ZUGFeRD-PDF geht ohne Sidecar nicht, weil
erst Mustang die XML-Datei aus dem PDF holt.

Bei Deinen eigenen Rechnungen gibt es diese Ausnahme nicht. Ausstellen
verlangt einen funktionierenden Sidecar, weil jede Rechnung als
geprüfte E-Rechnung entsteht.

## Fehler melden

Bleibt das Problem, melde es im Klein.Buch-Repository auf GitHub. Den
Link zum Quellcode findest Du im "Über"-Dialog. Füge die Log-Zeilen
rund um die Kennung bei und die Versionsnummer aus dem "Über"-Dialog.
Lies die Zeilen vorher durch und entferne Namen oder Beträge, falls
welche darin auftauchen.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
