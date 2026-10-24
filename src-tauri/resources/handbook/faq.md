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

Deine Daten sind dann unwiederbringlich verloren. Die Passphrase
heißt in der Oberfläche "Daten-Passwort" und ist zugleich der
Schlüssel Deiner Datenbank und Deiner Backups. Es gibt absichtlich
keine Hintertür. Einen Knopf zum Zurücksetzen gibt es in Klein.Buch
auch nicht. Ein Neuanfang geht nur mit einer neuen Instanz und einem
leeren Daten-Volume, die Du als Betreiber auf dem Server anlegst.
Notiere die Passphrase deshalb in einem Passwort-Manager.

Kennst Du die Passphrase noch und willst sie nur wechseln, geht das
unter "Einstellungen → Sicherheit & Anmeldung" im Abschnitt
"Daten-Passwort ändern".

## Ich habe das Handy mit der Authenticator-App verloren. Was nun?

Melde Dich mit einem Deiner zehn Einmal-Codes an, die Du bei der
Einrichtung bekommen hast. Dazu klickst Du in der Anmeldung auf
"Gerät verloren? Einmal-Code verwenden". Danach richtest Du unter "Einstellungen →
Sicherheit & Anmeldung" mit "Zweiten Faktor neu einrichten" Dein
neues Handy ein. Hast Du auch keinen Einmal-Code mehr, setzt Du den
zweiten Faktor als Betreiber über die Container-Shell zurück. Wie das
geht, steht im Kapitel "Zweiter Faktor (TOTP) einrichten und
wiederherstellen".
Deine Passphrase brauchst Du in jedem Fall.

## Kann ich Klein.Buch von mehreren Geräten aus nutzen?

Ja. Klein.Buch läuft auf Deinem Server, und Du öffnest es im Browser
von jedem Gerät aus: am Schreibtisch, auf dem Tablet oder auf dem
Handy. Alle Geräte sehen denselben Datenbestand, es muss nichts
abgeglichen werden. Eine Instanz ist für genau ein Unternehmen und
eine Person gedacht. Mehrere Benutzer mit eigenen Zugängen gibt es
nicht.

## Wie kommt meine Rechnung zum Steuerberater?

Am bequemsten über das Steuerberater-Paket. Klick im Hauptmenü auf
"Steuer (EÜR)", dann auf "Für die Steuererklärung exportieren →" und
bei "Steuerberater-Paket (ZIP, alles in einem)" auf "Paket
herunterladen". Klein.Buch fragt vorher noch einmal Dein
Daten-Passwort und den App-Code ab. Dein Browser speichert dann ein
ZIP mit PDFs, XRechnung-XMLs, DATEV-Stapel, ELSTER-Ausfüllhilfe als
CSV und einem Manifest mit SHA-256-Prüfwerten. Das ZIP sendest Du
Deinem Steuerberater per E-Mail oder Daten-Übergabe-Dienst. Mehr im
Kapitel "EÜR-Export".

## Darf ich als Kleinunternehmer Umsatzsteuer ausweisen?

Nein. Wenn Du es trotzdem tust, schuldest Du die ausgewiesene
Umsatzsteuer dem Finanzamt, auch wenn Du gar nicht
umsatzsteuerpflichtig bist (§14c UStG). Klein.Buch sperrt aus
diesem Grund alle Umsatzsteuer-Felder, solange der
Kleinunternehmer-Schalter angeschaltet ist.

## Kann ich Klein.Buch auf einen anderen Server umziehen?

Ja, über ein Backup. Du richtest auf dem neuen Server eine neue
Instanz ein und spielst dort unter "Einstellungen → Backups" im
Abschnitt "Wiederherstellen" die Backup-Datei (Endung `.kbk`) ein.
Dafür brauchst Du die Passphrase, mit der das Backup verschlüsselt
wurde. Backups einer älteren Klein.Buch-Version nimmt die neue
Instanz an und bringt sie auf den aktuellen Stand, Backups einer
neueren Version nicht. Die Einzelheiten stehen in den Kapiteln
"Backup und Wiederherstellen" und "Notfall: Instanz neu aufsetzen und
wiederherstellen".

## Warum lehnt Klein.Buch manche ZUGFeRD-Rechnungen ab?

Klein.Buch akzeptiert beim Empfang nur die ZUGFeRD-Profile (also
Format-Varianten), die seit 2025 das Gesetz als gültige E-Rechnung
verlangt: `EN16931`, `EXTENDED` und `XRECHNUNG`. Die alten Profile
`MINIMUM` und `BASIC-WL` lehnt Klein.Buch ab. Sie tragen nicht die
vollständigen Pflichtangaben einer Rechnung und gelten seit 2025 nur
noch als maschinenlesbare Beilage zu einer Papier- oder PDF-Rechnung.
Wenn ein Lieferant Dir so eine Datei schickt, frag ihn nach einer
richtigen E-Rechnung. Notfalls trägst Du die Position selbst über
"Kosten" und "+ Neue Kosten" ein.

## Wie kommen Rechnungen aus meinem Mail-Postfach in Klein.Buch?

Per Upload im Browser. Speichere den Anhang aus der E-Mail auf dem
Gerät, an dem Du gerade sitzt. Klick dann in der Kosten-Liste auf
"E-Rechnung importieren", wähle die Datei aus und klick auf
"Einlesen & prüfen". Einen Ordner, den Klein.Buch selbst überwacht,
gibt es nicht. Details im Kapitel "Rechnungseingang per Upload".

## Kann ich die Original-XML einer empfangenen Rechnung sehen?

Ja. Öffne den Eingangsbeleg in der Kosten-Liste, dort gibt es den
Button "Roh-XML anzeigen". Klein.Buch öffnet ein Fenster mit der
ursprünglichen XML-Datei, formatiert und mit Copy-Button. Das ist
praktisch, wenn Dein Steuerberater oder ein Prüfer in die
maschinenlesbaren Felder schauen will. Das Anzeigen ändert nichts am
Beleg.

## Was tun bei einer fehlgeschlagenen E-Rechnung-Validierung?

Klein.Buch zeigt die Meldungen des KoSIT-Validators. Bei Deinen
eigenen Rechnungen bricht das Ausstellen ab, wenn KoSIT die Rechnung
ablehnt; sie bleibt dann ein Entwurf. Fehlende Angaben wie die
Telefonnummer in Deinen Firmendaten oder das Fälligkeitsdatum meldet
Klein.Buch schon vorher mit einem klaren Satz. Kommt trotzdem eine
KoSIT-Ablehnung, ist das ein Klein.Buch-Fehler, den Du melden
solltest. Bei Eingangs-Rechnungen: Du erfasst die Datei trotzdem.
Der Fehler liegt beim Absender; Du bist nicht verpflichtet, eine
fehlerhafte Eingangs-XRechnung abzulehnen.

## Was ist der Unterschied zwischen Storno und Löschen?

Ein Storno ist eine Gegen-Buchung. Sie neutralisiert die
fehlerhafte Rechnung, lässt sie aber sichtbar. Löschen würde die
Original-Rechnung aus der Datenbank entfernen. Letzteres ist
gesetzlich verboten, weil das Finanzamt sonst nicht mehr
nachvollziehen könnte, was passiert ist. Klein.Buch erlaubt nur
das Storno.

## Wie oft macht Klein.Buch ein Backup?

Automatisch nach jeder Festschreibung (Lock-Event) und einmal pro
Tag im Hintergrund, solange Klein.Buch entsperrt ist. Zusätzlich
kannst Du jederzeit unter "Einstellungen → Backups" im Abschnitt
"Backup jetzt erstellen" mit "Jetzt sichern" eines auslösen. Jedes Backup geht an alle
eingerichteten externen Ziele. Scheitert ein Ziel, bekommst Du einen
Hinweis, und das nächste Backup versucht es wieder.

## Warum ist Klein.Buch nach einem Neustart gesperrt?

Weil Deine Passphrase nirgends gespeichert wird. Sie ist der
Schlüssel zur Datenbank. Nach jedem Neustart des Containers kann
Klein.Buch die Datenbank erst öffnen, wenn Du Dich im Browser mit
Daten-Passwort und App-Code anmeldest. Bis dahin laufen auch keine
Hintergrund-Aufgaben. Fällige Arbeiten wie das Tages-Backup oder
Abo-Rechnungen holt Klein.Buch nach der Anmeldung nach.

## Wie spiele ich ein Update ein?

Nicht in Klein.Buch selbst, sondern in Deiner Container-Verwaltung:
Du trägst die neue Versionsnummer als Image-Tag ein und startest den
Container neu. Vor einer nötigen Umstellung der Datenbank legt
Klein.Buch beim Entsperren selbst ein Backup an. Auf Wunsch meldet
Klein.Buch neue Versionen als Hinweis; den Schalter "Nach neuen
Versionen suchen" findest Du unter "Einstellungen → Sicherheit &
Anmeldung". Mehr im Kapitel "Updates und Rückfall".

## Wer hat Zugriff auf meine Daten?

Nur Du. Klein.Buch ist Local-First: Deine Daten liegen auf Deiner
eigenen Instanz, nicht bei einem Anbieter. Es gibt kein Konto bei
einem Hersteller und keinen Telemetrie-Kanal. Datenbank und Backups
sind verschlüsselt, und anmelden kann sich nur, wer Daten-Passwort
und App-Code hat. Bedenke aber: Wer Administrator-Rechte auf dem
Server hat, auf dem Klein.Buch läuft, kann eine entsperrte Instanz
grundsätzlich auslesen. Wähle den Server deshalb so sorgfältig wie
einen Aktenschrank.

## Kann ich Rechnungen in fremde Sprachen schreiben?

In Version 1.0 ist die Sprache des PDF-Templates Deutsch. Du
kannst die Texte in den Positionen frei wählen (also auch eine
englische Leistungs-Beschreibung), aber die Klein.Buch-Labels
("Rechnungs-Nummer", "Leistungsdatum") sind Deutsch. Volle
Mehrsprachigkeit kommt in einer späteren Version.

## Was bedeutet "Prüf- und Konvertierungsdienst nicht verfügbar"?

Klein.Buch startet im Container einen Java-Hilfsprozess für die
XRechnung-Erzeugung und -Prüfung (Mustang und KoSIT). Diesen Prozess
nennen wir Sidecar. Antwortet er nicht, kann Klein.Buch keine
Rechnung ausstellen und kein ZUGFeRD-PDF einlesen. Versuch es nach
einer Minute noch einmal. Hilft das nicht, lies das Kapitel
"Sidecar-Probleme" unter Troubleshooting.

## Wie sind meine Daten verschlüsselt?

Die Datenbank ist mit SQLCipher verschlüsselt (PBKDF2-HMAC-SHA512
für die Schlüsselableitung — ein Industrie-Standard-Verfahren).
Backups sind zusätzlich mit AES-256-GCM in einem Argon2id-
geschützten Container, ebenfalls Industrie-Standard. Konkret
heißt das: Ohne Deine Passphrase ist alles nur unleserlicher
Daten-Müll.

## Was kostet Klein.Buch?

Klein.Buch ist Open-Source-Software unter der AGPL-3.0-Lizenz.
Die Nutzung ist kostenlos. Betreibst Du Klein.Buch unverändert, ist
nichts weiter zu beachten. Veränderst Du Klein.Buch und lässt andere
Menschen Deine Fassung über das Netz nutzen, musst Du ihnen den
Quelltext dieser Fassung anbieten. Mehr im Kapitel "Lizenz und Quellcode: wenn Du Klein.Buch veränderst".

## Wo liegen meine Daten?

In den Volumes, die Du beim Einrichten des Containers angegeben
hast. Ein Volume ist ein Ordner auf dem Server, den der Container als
eigenes Verzeichnis sieht. Im Container heißen sie `/data` (die
verschlüsselte Datenbank, das Beleg-Archiv und die lokalen Backups),
`/inputs` (Vorlagen, Tabellen und Spezifikationen) und optional
`/backup-targets` (die externen Backup-Ziele). Das Daten-Volume
gehört auf ein lokales Laufwerk des Servers, nicht auf eine
Netzwerk-Freigabe und nicht in einen Cloud-Sync-Ordner. Mehr im
Kapitel "Volumes und Rechte".

## Wie melde ich einen Fehler?

Im Klein.Buch-Repository auf GitHub gibt es Issues. Dort beschreibst
Du Schritt für Schritt, was Du getan hast und was schief gegangen
ist. Hänge möglichst keine echten Kunden-Daten an; nutze
Demo-Daten oder schwärze Personen-Daten. Den Link zum Repository
findest Du im Hauptmenü unter "Über".

## Funktioniert Klein.Buch ohne Internet?

Teilweise. Dein Browser muss Deinen Server erreichen. Steht der
Server bei Dir im Haus, klappt das im eigenen Netz auch ohne
Internet. Internet braucht Klein.Buch selbst nur, wenn Du Rechnungen
per E-Mail versendest, Backups auf einen SFTP-Server außer Haus
schreibst oder die Versionsprüfung eingeschaltet hast. Und natürlich,
wenn Du unterwegs auf Deine Instanz zugreifen willst.

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

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
