---
slug: faq
title: Häufige Fragen
category: faq
order: 400
keywords: [faq, häufige fragen, antworten, hilfe, fragen, antworten]
---

# Häufige Fragen

Die häufigsten Fragen aus dem Alltag, kurz beantwortet. Wenn Deine
Frage nicht dabei ist, schau in die Kapitel "Bedienen", "Recht und
Steuern" oder "Troubleshooting".

## Wie ändere ich eine fertige Rechnung?

Gar nicht. Festgeschriebene Rechnungen sind nach GoBD unveränderlich.
Du erzeugst einen Storno-Beleg, der die fehlerhafte Rechnung
neutralisiert, und schreibst gegebenenfalls eine neue korrekte
Rechnung. Mehr im Kapitel "Storno statt Löschung".

## Was passiert, wenn ich meine Passphrase vergesse?

Deine Daten sind dann unwiederbringlich verloren. Es gibt
absichtlich keine Hintertür. Der einzige Weg zurück in die App ist
ein Factory Reset, der die gesamte Klein.Buch-Instanz auf
Werkszustand setzt und einen Daten-Neuanfang erzwingt. Notiere
die Passphrase in einem Passwort-Manager.

## Kann ich Klein.Buch auf mehrere Rechner verteilen?

Nein, nicht parallel. Klein.Buch ist Single-User-Software. Du
kannst die App auf einen neuen Rechner umziehen, indem Du das
aktuelle Backup mitnimmst und beim Onboarding statt einer leeren
Datenbank den Pfad zum Backup auswählst. Parallel-Nutzung würde die Datenbank zerstören.

## Wie kommt meine Rechnung zum Steuerberater?

Am bequemsten über das Steuerberater-Paket. Klick im Hauptmenü auf
"Einstellungen", dann "Daten exportieren" und löse den Export aus.
Klein.Buch packt PDFs, XRechnung-XMLs, DATEV-Stapel, ELSTER-
Ausfüllhilfe als CSV und Manifest mit SHA-256-Prüfwerten in ein
ZIP. Das ZIP sendest Du Deinem Steuerberater per E-Mail oder
Daten-Übergabe-Dienst.

## Darf ich als Kleinunternehmer Umsatzsteuer ausweisen?

Nein. Wenn Du es trotzdem tust, schuldest Du die ausgewiesene
Umsatzsteuer dem Finanzamt, auch wenn Du gar nicht
umsatzsteuerpflichtig bist (§14c UStG). Klein.Buch sperrt aus
diesem Grund alle Umsatzsteuer-Felder, solange der
Kleinunternehmer-Schalter angeschaltet ist.

## Kann ich die Datenbank zwischen Rechnern umziehen?

Ja, über die Backup-Funktion. Mache auf dem alten Rechner ein
Backup, kopiere die Backup-Datei auf den neuen Rechner, installiere
Klein.Buch dort und spiele das Backup zurück. Mehr im Kapitel
"Backup und Wiederherstellen".

## Warum lehnt Klein.Buch manche ZUGFeRD-Rechnungen ab?

Klein.Buch akzeptiert beim Empfang nur die ZUGFeRD-Profile (also
Format-Varianten), die seit 2025 das Gesetz als gültige E-Rechnung
verlangt: `EN16931`, `EXTENDED` und `XRECHNUNG`. Die alten Profile
`MINIMUM` und `BASIC-WL` lehnt Klein.Buch ab. Sie tragen nicht die
vollständigen Pflichtangaben einer Rechnung und gelten seit 2025 nur
noch als maschinenlesbare Beilage zu einer Papier- oder PDF-Rechnung.
Wenn ein Lieferant Dir so eine Datei schickt, frag ihn nach einer
richtigen E-Rechnung. Notfalls trägst Du die Position selbst über
"Kosten" und "Neue Kosten-Position" ein.

## Wie kommen Rechnungen aus meinem Mail-Postfach in Klein.Buch?

Auf zwei Arten.

**Manuell.** Lade den Anhang aus der E-Mail auf Deine Festplatte
(zum Beispiel in den Ordner "Downloads"). Klick dann in der
Kosten-Liste auf "E-Rechnung importieren" und wähle die
heruntergeladene Datei aus.

**Automatisch.** Richte einmalig den Rechnungs-Eingang in den
Einstellungen ein. Das ist ein Ordner auf Deiner Festplatte, den
Klein.Buch im Hintergrund überwacht. Speicherst Du Deine
Eingangsrechnungen direkt in diesen Ordner, übernimmt Klein.Buch
sie ohne weiteren Klick. Wenn Du dafür einen Cloud-Ordner wie
OneDrive nimmst, kannst Du Rechnungen auch vom Handy aus dort
hineinwerfen.

Details im Kapitel "Eingangsrechnungen via Ordner".

## Kann ich die Original-XML einer empfangenen Rechnung sehen?

Ja. Öffne den Eingangsbeleg in der Kosten-Liste, dort gibt es den
Button "Roh-XML anzeigen". Klein.Buch öffnet ein Fenster mit der
ursprünglichen XML-Datei, formatiert und mit Copy-Button. Das ist
praktisch, wenn Dein Steuerberater oder ein Prüfer in die
maschinenlesbaren Felder schauen will. Das Anzeigen ändert nichts am
Beleg.

## Was tun bei einer fehlgeschlagenen E-Rechnung-Validierung?

Klein.Buch zeigt die Fehlermeldung des KoSIT-Validators. Bei
Ausgangs-Rechnungen (also Deinen eigenen): Klein.Buch lässt das
Festschreiben in der Regel zu, weil Klein.Buch die Rechnung selbst
erzeugt und Standard-konform aufbaut. Wenn doch ein Fehler kommt,
ist das ein Klein.Buch-Bug, den Du melden solltest. Bei Eingangs-
Rechnungen: Du importierst die Datei trotzdem. Der Fehler liegt
beim Absender; Du bist nicht verpflichtet, eine fehlerhafte
Eingangs-XRechnung abzulehnen.

## Was ist der Unterschied zwischen Storno und Löschen?

Ein Storno ist eine Gegen-Buchung. Sie neutralisiert die
fehlerhafte Rechnung, lässt sie aber sichtbar. Löschen würde die
Original-Rechnung aus der Datenbank entfernen. Letzteres ist
gesetzlich verboten, weil das Finanzamt sonst nicht mehr
nachvollziehen könnte, was passiert ist. Klein.Buch erlaubt nur
das Storno.

## Wie oft macht Klein.Buch ein Backup?

Automatisch nach jeder Festschreibung (Lock-Event) und einmal pro
Tag beim App-Start. Zusätzlich kannst Du jederzeit manuell ein
Backup auslösen. Wenn ein Backup-Ziel zeitweise nicht erreichbar
ist, holt Klein.Buch es beim nächsten Versuch nach.

## Wer hat Zugriff auf meine Daten?

Niemand außer Dir. Klein.Buch ist Local-First. Es gibt keinen
zentralen Server, kein Konto bei einem Anbieter, keinen
Telemetrie-Kanal. Deine Daten liegen ausschließlich auf Deiner
Festplatte und in den Backup-Zielen, die Du selbst eingerichtet
hast.

## Kann ich Rechnungen in fremde Sprachen schreiben?

In Version 1.0 ist die Sprache des PDF-Templates Deutsch. Du
kannst die Texte in den Positionen frei wählen (also auch eine
englische Leistungs-Beschreibung), aber die Klein.Buch-Labels
("Rechnungs-Nummer", "Leistungsdatum") sind Deutsch. Volle
Mehrsprachigkeit kommt in einer späteren Version.

## Was bedeutet "Sidecar startet nicht"?

Klein.Buch startet im Hintergrund einen Java-Hilfsprozess für die
XRechnung-Erzeugung und -Prüfung (Mustang + KoSIT). Wenn der nicht
hochkommt, kann Klein.Buch keine XRechnung erzeugen oder prüfen.
Beende die App komplett und starte sie neu. Hilft das nicht, lies
das Kapitel "Sidecar-Probleme" unter Troubleshooting.

## Wie sind meine Daten verschlüsselt?

Die Datenbank ist mit SQLCipher verschlüsselt (PBKDF2-HMAC-SHA512
für die Schlüsselableitung — ein Industrie-Standard-Verfahren).
Backups sind zusätzlich mit AES-256-GCM in einem Argon2id-
geschützten Container, ebenfalls Industrie-Standard. Konkret
heißt das: Ohne Deine Passphrase ist alles nur unleserlicher
Daten-Müll.

## Was kostet Klein.Buch?

Klein.Buch ist Open-Source-Software unter der AGPL-3.0-Lizenz.
Die Nutzung ist kostenlos. Wenn Du die Software weiterentwickeln
oder selbst gehostet verbreiten willst, gelten die AGPL-Pflichten;
für die normale persönliche Nutzung ist nichts zu beachten.

## Wo finde ich die Programm-Daten?

Auf Windows liegen die Klein.Buch-Daten standardmäßig unter
`%APPDATA%\de.wildbach.kleinbuch\`. Dort befinden sich die
verschlüsselte Datenbank, das Beleg-Archiv und die Floor-Backups.
Achte darauf, dass dieser Ordner nicht mit einer Cloud
synchronisiert wird; das könnte die Datenbank korrumpieren. Den
Cloud-Sync nimmst Du nur für die separat eingerichteten
Off-Site-Backups.

## Wie melde ich einen Fehler?

Im Klein.Buch-Repository auf GitHub gibt es Issues. Dort beschreibst
Du Schritt für Schritt, was Du getan hast und was schief gegangen
ist. Hänge möglichst keine echten Kunden-Daten an; nutze
Demo-Daten oder schwärze Personen-Daten. Den Link zum Repository
findest Du im Hauptmenü unter "Über".

## Funktioniert Klein.Buch ohne Internet?

Ja. Die App ist offline-fähig. Internet brauchst Du nur, wenn Du
Rechnungen per E-Mail versenden oder Off-Site-Backups in die Cloud
beziehungsweise auf einen SFTP-Server schreiben willst. Sonst läuft
alles lokal.

## Kann ich mehrere Geschäftsjahre gleichzeitig verwalten?

Ja. Klein.Buch trennt alle Buchungen nach Geschäftsjahr und zeigt
die EÜR jeweils pro Jahr. Du kannst frei zwischen Jahren wechseln.
Solange ein Jahr nicht abgeschlossen ist, kannst Du dort noch
Belege nachtragen.

## Was wenn das Finanzamt die Datenbank sehen will?

Bei einer Betriebsprüfung darf das Finanzamt Einsicht verlangen.
Du übergibst dann den Steuerberater-ZIP. Klein.Buch lässt keine
Direkt-Anbindung an Prüfungs-Software zu (das nennt sich Z3-Zugriff
und ist die "Datenträger-Überlassung"). Der ZIP-Export erfüllt
diese Form: PDF, XML, DATEV-CSV und Inhalts-Übersicht.

## Was, wenn der Steuerberater eine andere Software nutzt?

Fast alle Steuerberater nutzen DATEV-kompatible Software. Der
DATEV-Stapel aus Klein.Buch funktioniert dort direkt. Wenn Dein
Berater eine andere Software hat, frag ihn nach dem akzeptierten
Format. Klein.Buch unterstützt in Version 1.0 keine weiteren
Format-Adapter, aber die Roh-PDFs und XMLs aus dem ZIP kann jede
seriöse Software lesen.

## Ist Klein.Buch GoBD-zertifiziert?

In Deutschland gibt es für Buchhaltungs-Software keine offizielle
GoBD-Zertifizierung. Es gibt nur die Möglichkeit, ein
GoBD-Testat (eine Bestätigung durch einen Wirtschafts-Prüfer) zu
erwerben. Klein.Buch in Version 1.0 hat noch kein solches Testat.
Die App ist nach den GoBD-Anforderungen gebaut, aber die formale
Bestätigung steht aus. Bei einer Prüfung wäge das mit Deinem
Steuerberater ab.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
