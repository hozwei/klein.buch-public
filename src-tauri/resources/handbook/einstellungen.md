---
slug: einstellungen
title: Einstellungen
category: bedienen
order: 240
keywords: [einstellungen, settings, firmendaten, mail, agb, datenschutz, konten, anfahrtspauschale, backup, audit, sicherheit, anmeldung, layout, daten exportieren, hinweis-regeln, e-mail-protokoll, backup-protokoll]
---

# Einstellungen

In den Einstellungen findest Du alle Konfigurationen, die Du nicht
jeden Tag brauchst. Klick im Hauptmenü auf "Einstellungen". Die
Bereiche stehen in drei Gruppen untereinander, jeder mit einer kurzen
Beschreibung:

1. "Firma und Belege": "Meine Firmendaten", "Konten",
   "Anfahrtspauschale", "Rechnungs-Layout" und "AGB & Datenschutz".
2. "Versand und Hinweise": "E-Mail-Versand", "E-Mail-Protokoll" und
   "Erinnerungen einstellen".
3. "Sicherheit und Daten": "Sicherheit & Anmeldung", "Backups",
   "Backup-Protokoll", "Protokoll & Datensicherheit", "Daten
   exportieren" und "EÜR-Export".

Klick einen Eintrag an, um den jeweiligen Bereich zu öffnen.

## Meine Firmendaten

Hier änderst Du Name, Rechtsform, Inhaber, Anschrift, Steuernummer
oder USt-IdNr., E-Mail und Telefon. Unter "Standard-Einstellungen"
legst Du fest, wie viele Tage ein neues Angebot gültig ist und welches
Zahlungsziel eine neue Rechnung bekommt. Bankverbindung und Logo
stehen nicht hier, sondern unter "Konten" und "Rechnungs-Layout".
Mehr dazu im Kapitel "Verkäuferprofil anlegen". Die Angaben speicherst
Du mit "Speichern" ohne Rückfrage.

Unter dem Formular steht der Abschnitt "Steuerstatus": Status heute,
Gesamtumsatz mit Grenzhinweisen und der Verlauf Deiner Wechsel
zwischen Kleinunternehmer-Regelung und Regelbesteuerung. Ohne Eintrag
gilt die Kleinunternehmer-Regelung. Einen Wechsel trägst Du dort mit
"Statuswechsel eintragen …" ein, mit Grund und Wirkungsdatum; er gilt
sofort, nicht erst mit "Speichern". Klein.Buch fragt dazu nach
Daten-Passwort und App-Code (nicht in den ersten fünf Minuten nach
der Anmeldung). Mehr im Kapitel "Kleinunternehmer-Regelung".

Klein.Buch schreibt zum Zeitpunkt jeder Festschreibung eine Kopie
der relevanten Profil-Daten in den Beleg. Spätere Profil-Änderungen
wirken nur auf neue Belege.

## Konten

Hier verwaltest Du Deine Zahlungskonten: Bankkonto, Bargeld-Kasse,
PayPal, Stripe und sonstige Zahlungswege. Diese Konten wählst Du aus,
wenn Du Zahlungen zu Rechnungen, Kosten und Privatbewegungen
erfasst. Konten mit "Auf Beleg anzeigen" erscheinen auf Angeboten
und Rechnungen. Mit den Postfächern unter "E-Mail-Versand" hat dieser
Bereich nichts zu tun.

Ein neues Konto legst Du unter "Neues Konto" mit "Konto anlegen" an.
"Bearbeiten" öffnet die Felder eines Kontos direkt in seiner Zeile,
"Änderungen speichern" übernimmt sie. Löschen lässt sich ein Konto
nicht. Du deaktivierst es im Menü "Mehr" der Zeile, ohne Rückfrage;
die Meldung danach bietet "Rückgängig" an. "Inaktive Konten anzeigen"
blendet deaktivierte Konten wieder ein. Anlegen, Ändern, Aktivieren
und Deaktivieren bestätigst Du mit Daten-Passwort und App-Code, weil
davon abhängt, welche Bankverbindung auf künftigen Rechnungen steht.
Mehr dazu im Kapitel "Konten und Mail-Versand".

## Anfahrtspauschale

Hier setzt Du unter "Preis pro Kilometer" den Satz für
Anfahrt-Positionen in Angeboten und Rechnungen. Mit "Standardmäßig
Hin & Rück (×2) vorschlagen" ist im Beleg das Häkchen "Hin & Rück"
schon gesetzt. "Speichern" übernimmt die Werte, "Abbrechen" führt
ohne Änderung zurück. Mehr dazu im Kapitel "Pakete und Anfahrt".

## Rechnungs-Layout

Oben lädst Du Dein Logo hoch (PNG, JPG, SVG, WebP oder GIF, höchstens
2 MB). Daneben kannst Du ein Bild Deiner Unterschrift hinterlegen; es
erscheint nur auf Angeboten über Deiner Unterschriftslinie. Der
Schalter "Unterschriftenfelder auf Angeboten anzeigen" blendet die
Unterschriftenfelder auf Angeboten ein oder aus.

Darunter wählst Du unter "Vorlage" das PDF-Layout für Deine Rechnungen
und Angebote. Mitgeliefert sind die Vorlagen "Standard", "Modern",
"Klassisch" und "Minimal". Alle enthalten dieselben Pflichtangaben
und die §19-Klausel; sie unterscheiden sich nur in Stil und Layout.
Jede Vorlage hat einen Knopf "Vorschau", der sie mit Beispiel-Daten
zeigt. "Als Standard wählen" macht eine Vorlage zur Vorgabe für neu
erzeugte Belege; die gewählte trägt "Ausgewählt". Bereits ausgestellte
Rechnungen bleiben unverändert.

In einer Rechnung oder einem Angebot steht im Feld "PDF-Vorlage"
zunächst "Wie in den Einstellungen (…)" mit dem Namen der hier
gewählten Vorlage. Dort kannst Du für einen einzelnen Beleg auch eine
andere Vorlage wählen.

### Eigene Vorlagen

Eigene Vorlagen legt der Betreiber als `.typ`-Datei in
`/inputs/pdf-templates/` ab. Sie erscheinen dann automatisch in der
Liste, gekennzeichnet als "Eigene Vorlage" (die mitgelieferten als
"Mitgeliefert"). Drei Dinge prüft Klein.Buch vor jedem Rendern:

1. **§19-Klausel.** Als Kleinunternehmer braucht die Vorlage den
   Kommentar `// §19-KLAUSEL-BLOCK: REQUIRED` und muss das Feld
   `kleinunternehmer.hinweis_text` ausgeben. Fehlt eines, lehnt
   Klein.Buch das Ausstellen ab. In der Liste trägt eine solche Vorlage
   "Ohne §19-Klausel" und lässt sich nicht als Standard wählen,
   solange Du Kleinunternehmer bist; eine passende trägt
   "§19-konform".
2. **Storno-Darstellung.** Einen Storno-Beleg erkennt die Vorlage am
   Feld `storno_of` (Nummer und Datum der stornierten Rechnung): Titel
   "Stornorechnung", Bezug auf die stornierte Rechnung, kein
   Überweisungshinweis. Nutzt Deine Vorlage `storno_of` nicht, trägt
   sie in der Liste "Ohne Storno-Darstellung" mit dem Hinweis, mit
   welcher Vorlage Klein.Buch Storno-Belege stattdessen erzeugt: mit
   der eingebauten Vorlage gleichen Namens, sonst mit "Klassisch".
   Beim Storno legt Klein.Buch dann einen Eintrag unter "Hinweise" an.
   So erscheint ein Storno nie als Rechnung mit Zahlungsaufforderung.
3. **Leistungszeitraum.** Trägt eine Rechnung statt eines
   Leistungsdatums einen Leistungszeitraum, zeigt die Vorlage ihn über
   die Felder `invoice.service_period_start` und
   `invoice.service_period_end` (Beginn und Ende). Nutzt Deine Vorlage
   diese Felder nicht, trägt sie in der Liste "Ohne Leistungszeitraum"
   mit dem Hinweis, mit welcher Vorlage Klein.Buch Rechnungen mit
   Zeitraum stattdessen erzeugt: wie beim Storno mit der eingebauten
   Vorlage gleichen Namens, sonst mit "Klassisch". Beim Ausstellen
   legt Klein.Buch dann einen Eintrag unter "Hinweise" an. So fehlt auf
   der Rechnung nie der Leistungszeitpunkt. Rechnungen mit
   Leistungsdatum erzeugt Deine Vorlage wie gewohnt.

Die mitgelieferte `default.typ` zeigt, wie alles aussieht.

Die Vorlagen laufen mit Typst 0.15. Die Belegdaten liest eine Vorlage
mit `json(bytes(sys.inputs.at("data-json")))` ein. Die frühere
Schreibweise `json.decode(…)` gibt es seit Typst 0.15 nicht mehr; eine
ältere eigene Vorlage bricht dann mit einem Hinweis auf genau diese
Zeile ab.

## AGB & Datenschutz

Hier verwaltest Du Versionen Deiner AGB und Deines
Datenschutzhinweises als PDF. Eine neue Version wählst Du unter "Neue
Version (PDF)" aus, lädst sie mit "Hochladen" hoch und schaltest sie
danach mit "Aktivieren" frei.
Beim Versand eines Angebots legt Klein.Buch die aktiven Fassungen bei
und verknüpft sie fest mit dem Angebot; zum Versenden brauchst Du je
eine aktive Fassung. Eine neue Version wird zu einem neuen Datensatz;
bereits an Angebote verknüpfte Versionen bleiben unverändert und
werden nie gelöscht.

## E-Mail-Versand

Hier richtest Du das Postfach für den Rechnungs-Versand ein. Klein.Buch
versendet über SMTP, auch ein Microsoft-365-Postfach. Über den Knopf
"E-Mail-Protokoll" oben auf der Seite kommst Du ins E-Mail-Protokoll.
Mehr dazu im Kapitel "Konten und Mail-Versand".

## E-Mail-Protokoll

Das E-Mail-Protokoll zeigt jeden Versandversuch mit Empfänger und
Ergebnis, auch die gescheiterten. Du durchsuchst es mit dem Suchfeld,
den Datumsfeldern "Von" und "Bis" und den Auswahlen "Status", "Art"
und "Kanal" (jeweils mit "Alle"). "Filtern" wendet die Angaben an,
"Filter zurücksetzen" hebt sie wieder auf. Ändern oder löschen lässt
sich das Protokoll nicht.

## Erinnerungen einstellen

Dieser Eintrag führt zur Seite "Erinnerungen einstellen". Dort
schaltest Du für jede Regel einzeln ein oder aus, welche Hinweise
erscheinen und wann, zum Beispiel die monatliche Beleg-Erinnerung
oder den Hinweis auf überfällige Rechnungen. Dieselbe Seite erreichst
Du im Hauptmenü unter "Hinweise" über den Knopf "Erinnerungen
einstellen". Mehr dazu im Kapitel "Hinweise".

## Sicherheit & Anmeldung

Hier erzeugst Du neue Einmal-Codes, richtest den zweiten Faktor
(Authenticator-App) neu ein, änderst Dein Daten-Passwort und
schaltest die Versionsprüfung ein oder aus. Die Versionsprüfung fragt
auf Wunsch höchstens einmal täglich bei GitHub nach einer neuen
Version und legt dann einen Hinweis an. Neue Einmal-Codes, einen neuen
zweiten Faktor und ein neues Daten-Passwort bestätigst Du vorher mit
Daten-Passwort und App-Code.

Unter "Angemeldete Sitzungen" siehst Du jede Anmeldung mit Zeitpunkt,
letzter Nutzung und einer kurzen Kennung; Deine eigene ist als "Diese
Sitzung" markiert. "Andere Sitzungen abmelden" beendet nach einer
Bestätigung alle übrigen Anmeldungen sofort.

Einen Bereich zum Zurücksetzen oder Löschen aller Daten gibt es
nicht. Über das Netz wäre das die gefährlichste einzelne Aktion. Wer
neu anfangen will, legt als Betreiber eine neue Instanz mit leerem
Daten-Volume an.

## Backups

Hier richtest Du die externen Backup-Ziele ein, auch mehrere
gleichzeitig: Ordner unter `/backup-targets`, die Du als Betreiber in
den Container einbindest (zum Beispiel eine NAS-Freigabe oder den
Ordner eines Sync-Programms), und SFTP-Server. Außerdem löst Du hier
mit "Jetzt sichern" eine Sicherung aus, stellst die Uhrzeit des täglichen
Backups ein (Standard 02:30), siehst den Backup-Verlauf, stellst ein Backup
wieder her und siehst unter "Nummernkreise" die zuletzt vergebenen Nummern
(Anheben nach einer Wiederherstellung in eine neue Instanz). Über den
Knopf "Backup-Protokoll" oben auf der Seite oder den gleichnamigen
Eintrag in den Einstellungen kommst Du ins Backup-Protokoll mit
Zeitpunkt, Status, Auslöser, Ziel, Datei und Größe jeder Sicherung.
Es lässt sich wie das E-Mail-Protokoll durchsuchen und filtern. Mehr
dazu im Kapitel "Backup und Wiederherstellen".

Die lokale Sicherung läuft automatisch und ist nicht
konfigurierbar. Sie liegt im Daten-Volume des Servers, also auf
demselben Speicher wie Deine Daten. Deshalb brauchst Du zusätzlich
mindestens ein externes Ziel.

## Protokoll & Datensicherheit

Hier siehst Du das Audit-Protokoll: die unveränderliche Liste
aller buchhalterisch relevanten Aktionen (jede Festschreibung,
jede Zahlung, jeder Storno, jede DSGVO-Auskunft, jeder
Geschäftsjahres-Abschluss). Es zeigt die jüngsten Einträge, die
neuesten zuerst, mit Zeitpunkt, Aktion, "Ausgelöst von" ("Du",
"Zeitplan" oder "System") und Bezug. Bearbeiten oder löschen lässt
es sich nicht. Im selben Bereich prüfst Du unter "Archiv-Integrität"
mit "Jetzt prüfen" das Beleg-Archiv (SHA-256-Vergleich gegen die
Datenbank); automatisch läuft diese Prüfung einmal im Monat. Mehr
Hintergrund im Kapitel "GoBD im Alltag".

## Daten exportieren

Hier speicherst Du mit "Export erstellen und herunterladen" alle
Deine Daten in einer ZIP-Datei: Rechnungen, Kosten, Belege, Kontakte
und Einstellungen. So kommst Du jederzeit an Deine Daten, auch
unabhängig von Klein.Buch, zum Beispiel für einen Programmwechsel.
Vorher bestätigst Du mit Daten-Passwort und App-Code. Dein Browser
speichert die Datei in seinem Download-Ordner. Der Download lässt
sich nur einmal abrufen; bricht er ab, erstellst Du den Export neu.
Die Datei ist nicht verschlüsselt, bewahre sie sicher auf. Passwörter
von Postfächern und Backup-Zielen sind nicht enthalten.

Das Steuerberater-Paket findest Du nicht hier, sondern unter "Steuer
(EÜR)" im Export-Bereich; der Eintrag "EÜR-Export" in den
Einstellungen führt direkt dorthin. Mehr dazu im Kapitel "EÜR-Export".

## Was Du nicht hier findest

Zwei Dinge tauchen oft als Suchwunsch in den Einstellungen auf,
sitzen aber woanders:

**Der Über-Dialog** mit Klein.Buch-Version, Lizenz, dem Link zum
Quellcode dieser Version und den Drittanbieter-Lizenzen sitzt als
eigener Eintrag "Über" im Hauptmenü unten neben "Hilfe".

**Abmelden und Datenbank sperren** sitzen ebenfalls unten im
Hauptmenü. "Abmelden" beendet nur die Sitzung in diesem Browser.
"Datenbank sperren" schließt nach einer Rückfrage die Datenbank und
meldet alle Browser ab; danach braucht es wieder Daten-Passwort und
App-Code.

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
