---
slug: einstellungen
title: Einstellungen
category: bedienen
order: 240
keywords: [einstellungen, settings, firmendaten, mail, agb, datenschutz, konten, anfahrtspauschale, backup, audit, sicherheit, anmeldung, layout, daten exportieren]
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

Änderst Du Bankverbindung (IBAN, BIC) oder den Kleinunternehmer-Status,
fragt Klein.Buch zur Bestätigung nach Daten-Passwort und App-Code
(nicht in den ersten fünf Minuten nach der Anmeldung). Adresse,
Steuernummer und Logo speicherst Du ohne Rückfrage.

Klein.Buch schreibt zum Zeitpunkt jeder Festschreibung eine Kopie
der relevanten Profil-Daten in den Beleg. Spätere Profil-Änderungen
wirken nur auf neue Belege.

## E-Mail-Versand

Hier richtest Du das Postfach für den Rechnungs-Versand ein. Klein.Buch
versendet über SMTP, auch ein Microsoft-365-Postfach. Oben rechts
kommst Du ins E-Mail-Protokoll mit allen Versandversuchen. Mehr dazu
im Kapitel "Konten und Mail-Versand".

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

## Rechnungs-Layout

Hier wählst Du eines der mitgelieferten PDF-Templates für Deine
Rechnungen und Angebote. Alle Templates enthalten dieselben
Pflichtangaben und die §19-Klausel; sie unterscheiden sich nur in
Stil und Layout. Eine Vorschau zeigt das gewählte Template mit
Beispiel-Daten.

### Eigene Vorlagen

Eigene Vorlagen legt der Betreiber als `.typ`-Datei in
`/inputs/pdf-templates/` ab. Zwei Dinge prüft Klein.Buch vor jedem
Rendern:

1. **§19-Klausel.** Als Kleinunternehmer braucht die Vorlage den
   Kommentar `// §19-KLAUSEL-BLOCK: REQUIRED` und muss das Feld
   `kleinunternehmer.hinweis_text` ausgeben. Fehlt eines, lehnt
   Klein.Buch das Ausstellen ab.
2. **Storno-Darstellung.** Eine Storno-Rechnung erkennt die Vorlage am
   Feld `storno_of` (Nummer und Datum der stornierten Rechnung): Titel
   "Stornorechnung", Bezug auf die stornierte Rechnung, kein
   Überweisungshinweis. Nutzt Deine Vorlage `storno_of` nicht, erzeugt
   Klein.Buch Stornos mit der eingebauten Vorlage "Klassisch" (bzw. der
   eingebauten gleichen Namens) und legt einen Hinweis in die Inbox. So
   erscheint ein Storno nie als Rechnung mit Zahlungsaufforderung. Die
   mitgelieferte `default.typ` zeigt, wie beides aussieht.

## Backups

Hier richtest Du die externen Backup-Ziele ein, auch mehrere
gleichzeitig: Ordner unter `/backup-targets`, die Du als Betreiber in
den Container einbindest (zum Beispiel eine NAS-Freigabe oder den
Ordner eines Sync-Programms), und SFTP-Server. Außerdem löst Du hier
mit "Jetzt sichern" eine Sicherung aus, stellst die Uhrzeit des täglichen
Backups ein (Standard 02:30), siehst den Backup-Verlauf, stellst ein Backup
wieder her und siehst unter "Nummernkreise" die zuletzt vergebenen Nummern
(Anheben nach einer Wiederherstellung in eine neue Instanz). Über die PageBar
oben rechts kommst Du ins Backup-Protokoll mit Zeitpunkt, Status,
Auslöser, Ziel, Datei und Größe jeder Sicherung. Mehr dazu im Kapitel
"Backup und Wiederherstellen".

Die lokale Sicherung läuft automatisch und ist nicht
konfigurierbar. Sie liegt im Daten-Volume des Servers, also auf
demselben Speicher wie Deine Daten. Deshalb brauchst Du zusätzlich
mindestens ein externes Ziel.

## Daten exportieren

Hier speicherst Du mit "Export erstellen und herunterladen" alle
Deine Daten in einer ZIP-Datei: Rechnungen, Kosten, Belege, Kontakte
und Einstellungen. So kommst Du jederzeit an Deine Daten, auch
unabhängig von Klein.Buch, zum Beispiel für einen Programmwechsel.
Dein Browser speichert die Datei in seinem Download-Ordner. Die Datei
ist nicht verschlüsselt, bewahre sie sicher auf. Passwörter von
Postfächern und Backup-Zielen sind nicht enthalten.

Das Steuerberater-Paket findest Du nicht hier, sondern unter "Steuer
(EÜR)" im Export-Bereich. Mehr dazu im Kapitel "EÜR-Export".

## Protokoll & Datensicherheit

Hier öffnest Du das Audit-Log: die unveränderliche Protokoll-Liste
aller buchhalterisch relevanten Aktionen (jede Festschreibung,
jede Zahlung, jeder Storno, jede DSGVO-Auskunft, jeder
Geschäftsjahres-Abschluss). Das Log lässt sich filtern und
durchsuchen, aber weder bearbeiten noch löschen. Im selben Bereich
prüfst Du die Integrität des Beleg-Archivs (SHA-256-Vergleich
gegen die Datenbank). Mehr Hintergrund im Kapitel "GoBD im Alltag".

## Sicherheit & Anmeldung

Hier erzeugst Du neue Einmal-Codes, richtest den zweiten Faktor
(Authenticator-App) neu ein, änderst Dein Daten-Passwort und
schaltest die Versionsprüfung ein oder aus. Die Versionsprüfung fragt
auf Wunsch höchstens einmal täglich bei GitHub nach einer neuen
Version und legt dann einen Hinweis an. Neue Einmal-Codes, einen neuen
zweiten Faktor und ein neues Daten-Passwort bestätigst Du vorher mit
Daten-Passwort und App-Code.

Unter "Angemeldete Sitzungen" siehst Du jede Anmeldung mit Zeitpunkt,
letzter Nutzung und einer kurzen Kennung; Deine eigene ist als "diese
Sitzung" markiert. "Andere Sitzungen abmelden" beendet nach einer
Bestätigung alle übrigen Anmeldungen sofort.

Einen Bereich zum Zurücksetzen oder Löschen aller Daten gibt es
nicht. Über das Netz wäre das die gefährlichste einzelne Aktion. Wer
neu anfangen will, legt als Betreiber eine neue Instanz mit leerem
Daten-Volume an.

## Was Du nicht hier findest

Drei Dinge tauchen oft als Suchwunsch in den Einstellungen auf,
sitzen aber woanders:

**Hinweis-Regeln** stellst Du nicht hier ein, sondern im
Hauptmenü unter "Hinweise" über den Knopf "Erinnerungen
einstellen".

**Der Über-Dialog** mit Klein.Buch-Version, Lizenz, dem Link zum
Quellcode dieser Version und den Drittanbieter-Lizenzen sitzt als
eigener Eintrag "Über" im Hauptmenü unten neben "Hilfe".

**Abmelden und Datenbank sperren** sitzen ebenfalls unten im
Hauptmenü. "Abmelden" beendet nur die Sitzung in diesem Browser.
"Datenbank sperren" schließt die Datenbank und meldet alle Browser
ab; danach braucht es wieder Daten-Passwort und App-Code.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
