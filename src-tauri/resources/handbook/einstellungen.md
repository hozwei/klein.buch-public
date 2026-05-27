---
slug: einstellungen
title: Einstellungen
category: bedienen
order: 240
keywords: [einstellungen, settings, firmendaten, mail, agb, datenschutz, konten, anfahrtspauschale, backup, audit, rechnungs-eingang, layout, daten exportieren, zurücksetzen]
---

# Einstellungen

In den Einstellungen findest Du alle Konfigurationen, die Du nicht
jeden Tag brauchst. Klick im Hauptmenü auf "Einstellungen". Die
Bereiche stehen als Karten-Übersicht. Klick eine Karte an, um den
jeweiligen Bereich zu öffnen.

## Meine Firmendaten

Hier änderst Du Adresse, Steuernummer, Bankverbindung und Logo. In
diesem Bereich sitzt auch der Schalter "Ich bin Kleinunternehmer
nach §19 UStG". Standardmäßig ist er an. Mehr dazu im Kapitel
"Verkäuferprofil anlegen".

Klein.Buch schreibt zum Zeitpunkt jeder Festschreibung eine Kopie
der relevanten Profil-Daten in den Beleg. Spätere Profil-Änderungen
wirken nur auf neue Belege.

## E-Mail-Versand

Hier richtest Du das Postfach für den Rechnungs-Versand ein
(SMTP oder Microsoft 365). Im selben Bereich liegt das
Versand-Protokoll mit allen verschickten Mails. Mehr dazu im
Kapitel "Konten und Mail-Versand".

## AGB & Datenschutz

Hier verwaltest Du Versionen Deiner AGB und Deines
Datenschutzhinweises. Eine neue Version wird zu einem neuen
Datensatz; bereits an Angebote oder Rechnungen verknüpfte
Versionen bleiben unverändert.

## Konten

Hier verwaltest Du Deine Zahlungs-Konten: Bankkonto, Bargeld und
weitere Zahlungswege. Diese Konten erscheinen in den Drop-downs
bei Rechnungen, Kosten und Privatbewegungen. Mit "E-Mail-Versand"
oben hat dieser Bereich nichts zu tun, auch wenn beides
"Konten" im Namen trägt.

## Anfahrtspauschale

Hier setzt Du den Kilometer-Satz für Anfahrt-Positionen in
Angeboten und Rechnungen. Du kannst auch festlegen, ob Hin- und
Rückfahrt automatisch doppelt gerechnet werden. Mehr dazu im
Kapitel "Pakete und Anfahrt".

## Rechnungs-Eingang

Hier richtest Du einen Ordner ein, den Klein.Buch im Hintergrund
auf neue Eingangsrechnungen prüft. Du wählst einen Pfad und
schaltest den Toggle ein. Sobald Du dann eine XRechnung-Datei oder
eine ZUGFeRD-PDF in den Ordner legst, übernimmt Klein.Buch sie
automatisch. Klein.Buch muss dafür laufen und entsperrt sein.
Mehr dazu im Kapitel "Eingangsrechnungen via Ordner".

## Rechnungs-Layout

Hier wählst Du eines der mitgelieferten PDF-Templates für Deine
Rechnungen und Angebote. Alle Templates enthalten dieselben
Pflichtangaben und die §19-Klausel; sie unterscheiden sich nur in
Stil und Layout. Eine Vorschau zeigt das gewählte Template mit
Beispiel-Daten.

## Backups

Hier richtest Du das Off-Site-Ziel ein (Cloud-Ordner oder
SFTP-Server), siehst die Retention-Mengen und löst manuelle
Sicherungen aus. Über die PageBar oben rechts kommst Du ins
Backup-Protokoll mit Datum, Größe, Pfad und Status jeder
Sicherung. Mehr dazu im Kapitel "Backup und Wiederherstellen".

Die lokale Sicherung läuft automatisch und ist nicht
konfigurierbar — sie liegt im Klein.Buch-Daten-Ordner.

## Daten exportieren

Hier erzeugst Du das Steuerberater-Paket als ZIP: EÜR-PDF,
DATEV-Buchungsstapel, ELSTER-Ausfüllhilfe (CSV), alle Rechnungen
als PDF und XRechnung-XML, alle Kosten-Belege, eine
Inhalts-Übersicht und ein Manifest mit SHA-256-Prüfwerten pro
Datei. Mehr dazu im Kapitel "EÜR-Export".

## Protokoll & Datensicherheit

Hier öffnest Du das Audit-Log: die unveränderliche Protokoll-Liste
aller buchhalterisch relevanten Aktionen (jede Festschreibung,
jede Zahlung, jeder Storno, jede DSGVO-Auskunft, jeder
Geschäftsjahres-Abschluss). Das Log lässt sich filtern und
durchsuchen, aber weder bearbeiten noch löschen. Im selben Bereich
prüfst Du die Integrität des Beleg-Archivs (SHA-256-Vergleich
gegen die Datenbank). Mehr Hintergrund im Kapitel "GoBD im Alltag".

## Zurücksetzen

Hier löschst Du alle lokalen Daten und beginnst neu, zum Beispiel
zur Geräte-Weitergabe. Vor dem Reset musst Du entweder das
Steuerberater-Paket exportiert haben oder eine getippte
Aufbewahrungs-Quittung bestätigen. Mehr dazu im Kapitel
"Zurücksetzen".

## Was Du nicht hier findest

Drei Dinge tauchen oft als Suchwunsch in den Einstellungen auf,
sitzen aber woanders:

**Hinweis-Regeln** stellst Du nicht hier ein, sondern im
Hauptmenü unter "Hinweise" über den Knopf "Erinnerungen
einstellen".

**Der Über-Dialog** mit Klein.Buch-Version, Daten-Pfad,
Sidecar-Status und Drittanbieter-Lizenzen sitzt als eigener
Eintrag "Über" im Hauptmenü unten neben "Hilfe".

**Die Passphrase ändern** geht in Klein.Buch v1.0 nicht. Wenn Du
sie wechseln musst, exportierst Du das Steuerberater-Paket,
setzt Klein.Buch über "Zurücksetzen" zurück und richtest die App
mit neuer Passphrase neu ein.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
