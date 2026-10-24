# Klein.Buch — Benutzerhandbuch (Kurzfassung)

Das vollständige Handbuch ist in Klein.Buch eingebaut: im Menü unter **Hilfe**
(oder mit F1), mit Volltext-Suche und Glossar. Die Kapitel liegen auch im
Repository unter `src-tauri/resources/handbook/` (eine Markdown-Datei je
Kapitel). Diese Datei gibt nur einen Überblick über die Abläufe und verweist
dorthin; maßgeblich ist das eingebaute Handbuch.

Klein.Buch läuft als selbst gehosteter Dienst und wird im Browser bedient, auf
dem Computer wie auf dem Telefon. Installation und Betrieb (Docker, Reverse-Proxy,
Volumes, Backups, Updates) beschreiben das Betriebshandbuch
`docs/operations-guide.md` und die Handbuch-Kapitel `installation-docker`,
`reverse-proxy-und-tls`, `volumes-und-rechte`,
`totp-einrichten-und-wiederherstellen`, `updates-und-rueckfall`,
`disaster-recovery`, `speicherbedarf` und `fehlersuche-im-container`.

Klein.Buch ist ein Werkzeug, kein Steuerberater — die Pflichtangaben- und
EÜR-Logik sollte vor dem Echtbetrieb von einem Steuerberater gegengeprüft
werden.

## 1. Erste Schritte

### 1.1 Einrichten und Anmelden

Beim ersten Aufruf richtet der Betreiber bzw. Nutzer die Instanz im Browser ein:
**Setup-Token** aus dem Container-Log, **Daten-Passwort** (mindestens 16
Zeichen) und eine **Authenticator-App** für den zweiten Faktor (QR-Code
scannen, Code bestätigen). Dazu gibt es zehn **Einmal-Codes** für den Fall, dass
das Gerät mit der App verloren geht.

Das Daten-Passwort verschlüsselt Datenbank und Backups und wird **nirgends
gespeichert**. Geht es verloren, sind Daten und Backups unbrauchbar — es gibt
keinen Wiederherstellungsweg. Bewahre es in einem Passwort-Manager auf.

Angemeldet wird mit Daten-Passwort und App-Code. Nach jedem Neustart des
Servers ist Klein.Buch gesperrt, bis sich jemand anmeldet; erst dann laufen
Tages-Backup, Abo-Rechnungen und die übrigen geplanten Aufgaben. Für kritische
Aktionen (Wiederherstellen, Backup-Ziele, Mail-Postfächer, vollständige Exporte,
Passwort- oder App-Wechsel) fragt Klein.Buch beide Faktoren erneut ab.
Einstellungen dazu: **Einstellungen → Sicherheit & Anmeldung**.

### 1.2 Firmendaten

Unter **Einstellungen → Meine Firmendaten** trägst du dein Unternehmen ein: Name,
Adresse, Steuernummer bzw. USt-IdNr., Telefon, E-Mail, Zahlungsziel.
Bankverbindung und weitere Zahlwege pflegst du unter **Einstellungen → Konten**,
Logo und Unterschrift unter **Einstellungen → Rechnungs-Layout**. Ohne
Telefonnummer und ohne Steuernummer bzw. USt-IdNr. lässt sich keine Rechnung
ausstellen (die E-Rechnung verlangt beides).

Der Schalter **Kleinunternehmer (§19)** ist standardmäßig aktiv. In diesem Modus
weist Klein.Buch keine Umsatzsteuer aus und setzt automatisch den Pflichthinweis
„Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen." auf jede Rechnung und in die
E-Rechnung. Ein Verzicht auf §19 (Wechsel zur Regelbesteuerung) ist möglich,
bindet dich aber 5 Jahre — Klein.Buch warnt dich vor dem Umschalten.

### 1.3 E-Mail-Versand

Unter **Einstellungen → E-Mail-Versand** legst du ein Postfach an: Absender,
SMTP-Server und -Port, Benutzername und das **Passwort deines Mail-Postfachs**
(bei Zwei-Faktor-Anmeldung des Anbieters ein App-Passwort). Port **587** heißt
STARTTLS, Port **465** implizites TLS; ohne Verschlüsselung geht es nur ohne
Benutzer und Passwort.

- Das Passwort liegt verschlüsselt in der Klein.Buch-Datenbank und gilt nur für
  den eingetragenen Server, Port und Benutzer; wer eins davon ändert, gibt das
  Passwort neu ein.
- Verbindungstest und Testmail prüfen den Versandweg (nur auf üblichen
  Mail-Ports, höchstens zehn Tests je Minute).
- **Microsoft 365** wird über SMTP angebunden (z. B. `smtp.office365.com`, Port
  587, sofern der Anbieter SMTP-Versand erlaubt). Die frühere Microsoft-Anmeldung
  über OAuth gibt es im Server nicht.
- Jeder Versand steht im **Versandprotokoll** (gleiche Seite), auch
  Fehlschläge.

> Hinweis zur Zustellung: Dass eine Mail vom Provider angenommen wird, heißt
> nicht, dass sie beim Empfänger im Posteingang landet. Für zuverlässige
> Zustellung an Geschäftskunden versendest du am besten von einer **eigenen
> Domain** mit korrekt eingerichtetem SPF, DKIM und DMARC.

## 2. Rechnungen

- **Kontakt anlegen** unter **Kontakte**; die E-Mail wird beim Versand
  vorbelegt.
- **Rechnung anlegen** unter **Rechnungen**: Kontakt, Positionen (auch Pakete
  und Anfahrt), Fälligkeitsdatum. Im §19-Modus sind die USt-Felder gesperrt.
  Gespeichert wird ein **Entwurf** mit bereits vergebener Nummer
  (`RE-{Jahr}-{Nummer}`); das Geschäftsjahr ergibt sich aus dem Rechnungsdatum.
- **Prüfen** und **Rechnung ausstellen**: Klein.Buch erzeugt die XRechnung, lässt
  sie vom KoSIT-Validator prüfen, erzeugt das ZUGFeRD-PDF/A-3, archiviert beides
  unveränderbar und legt ein Backup an. Ab jetzt ist die Rechnung
  **festgeschrieben**; Korrekturen laufen nur über **Stornieren**
  (Storno-Beleg `ST-{Jahr}-{Nummer}`), nie über Löschen.
- **PDF öffnen** zeigt das PDF im Browser (auf dem Telefon in der PDF-Ansicht des
  Browsers), **PDF herunterladen** speichert es.
- **Senden** verschickt das ZUGFeRD-PDF über dein Postfach; Empfänger, Betreff
  und Text sind vorbelegt und änderbar.
- **+ Zahlung** erfasst Zahlungseingänge mit Datum; für die EÜR zählt das
  Zahlungsdatum (Cash-Basis), Teilzahlungen auch über Jahresgrenzen.
- **Wiederkehrende Rechnungen** (Button auf der Rechnungen-Seite): Abo-Vorlagen
  mit Frequenz und Stichtag; je Vorlage nur Entwurf, automatisch ausstellen oder
  ausstellen und senden. Das Belegdatum ist immer der Erstellungstag; verpasste
  Perioden holt Klein.Buch nach.

## 3. Angebote

Eigener Belegkreis (`AN-{Jahr}-{Nummer}`) und Lebenszyklus: *Entwurf →
Versendet → Angenommen/Abgelehnt → Umgewandelt/Storniert*. Festgeschriebene
Angebote sind unveränderlich. **PDF anzeigen** zeigt das Angebot, **Druck-PDF
(mit AGB)** fügt Angebot, AGB und Datenschutz zu einem PDF zusammen; der Versand
schickt sie als drei Anhänge. Beim **Annehmen** kannst du den unterschriebenen
Vertrag hochladen; er wird unveränderbar archiviert. **In Rechnung umwandeln**
erzeugt aus einem angenommenen Angebot einen Rechnungs-Entwurf.

AGB und Datenschutz pflegst du unter **Einstellungen → AGB & Datenschutz** als
versionierte PDF-Dokumente; je Typ ist genau eine Version aktiv, alte Fassungen
bleiben als Nachweis erhalten und werden nie gelöscht.

## 4. Kosten, E-Rechnungen, Privat-Geld

- **+ Neue Kosten**: Lieferant, Datum, Kategorie, Beschreibung, Betrag (brutto),
  optional der Beleg als PDF oder Bild — auf dem Telefon auch direkt mit der
  Kamera („Beleg fotografieren"). Kosten sind sofort festgeschrieben; Korrektur
  über Stornieren. Für die EÜR zählt das Zahlungsdatum.
- **E-Rechnung importieren**: empfangene XRechnung (XML, CII oder UBL) oder
  ZUGFeRD-PDF hochladen. Klein.Buch liest die Daten, prüft sie mit dem
  KoSIT-Validator (Hinweis, kein Hindernis), archiviert das Original und legt
  eine Kostenposition an. Angenommen werden die ZUGFeRD-Profile `EN16931`,
  `EXTENDED` und `XRECHNUNG`; `MINIMUM` und `BASIC-WL` sind keine gültigen
  E-Rechnungen und werden abgelehnt. Die Roh-XML einer empfangenen Rechnung
  zeigt die Detailseite.
- **Wiederkehrende Abos** für regelmäßige Kosten (Miete, Software).
- **Privat-Geld** dokumentiert Entnahmen und Einlagen (EÜR-neutral).

## 5. Anschaffungen und Abschreibung

Unter **Anschaffungen** erfasst du Anlagen über der GWG-Grenze mit Methode und
betrieblichem Anteil; **AfA buchen** erzeugt die Abschreibung je Geschäftsjahr
(wiederholbar). Eine Veräußerung löst den Restbuchwert auf. Die AfA-Sätze
kommen aus der Tabelle `afa-tabellen.json` im Volume `/inputs`.

## 6. EÜR und Steuer-Export

Unter **Steuer (EÜR)** siehst du die Einnahmen-Überschuss-Rechnung eines Jahres
auf Cash-Basis. **Für die Steuererklärung exportieren** erzeugt die
ELSTER-Ausfüllhilfe mit „Anlage EÜR"-PDF, den DATEV-Buchungsstapel (SKR03,
umschaltbar auf SKR04) und das Steuerberater-Paket (ZIP). Alle Exporte kommen als
Download in den Browser. Die Konten-Zuordnung im DATEV-Export ist ein Vorschlag
und vom Steuerberater zu prüfen.

## 7. Geschäftsjahr und Hinweise

Unter **Geschäftsjahr** schließt du ein **abgelaufenes** Jahr ab: AfA buchen,
Jahr prüfungssicher festschreiben (§146 AO), Backup. Der Abschluss ist
unumkehrbar; Korrekturen danach nur per Storno-Beleg. Die automatische AfA zum
Jahreswechsel ist standardmäßig aktiv und dort abschaltbar.

**Hinweise** erinnern an Fälliges (überfällige Rechnungen, Backups, fehlende oder
gescheiterte externe Backup-Ziele, Geschäftsjahr abschließen) und — wenn unter
**Einstellungen → Sicherheit & Anmeldung** eingeschaltet — an neue Versionen.
Welche Erinnerungen aktiv sind, steuerst du unter **Hinweise → Erinnerungen
einstellen**.

## 8. Backups und Wiederherstellen

Unter **Einstellungen → Backups**:

- **Status** und **Backup-Verlauf**; **Backup jetzt erstellen**.
- **Externe Ziele**: Ordner unter `/backup-targets` (vom Betreiber
  eingebunden, z. B. NAS oder Sync-Ordner) und SFTP-Server, gern mehrere.
  Backups sind verschlüsselt und liegen auch bei einem Anbieter sicher. Ohne
  externes Ziel liegen alle Backups auf demselben Server wie die Daten.
- **Wiederherstellen**: Backup-Datei hochladen oder ein lokales Backup wählen und
  das Daten-Passwort dieses Backups eingeben. Klein.Buch sichert vorher den
  aktuellen Stand und startet danach neu; anschließend gilt das Daten-Passwort
  des Backups.

Automatische Backups entstehen bei jedem Festschreiben und einmal täglich, vor
jeder Wiederherstellung und vor jeder Datenbank-Umstellung durch ein Update.
Eigene Vorlagen in `/inputs` sind **nicht** Teil der Backups; die sichert der
Betreiber.

**Alles löschen** gibt es in Klein.Buch nicht (zehnjährige Aufbewahrungspflicht,
§147 AO). Wer neu beginnen will, lässt den Betreiber eine neue Instanz mit leerem
Datenverzeichnis anlegen.

## 9. Daten exportieren und DSGVO

**Einstellungen → Daten exportieren** erzeugt ein offenes ZIP mit allen Daten
(eine JSON-Datei je Tabelle, Belegarchiv, Schema, Lesehilfe `read_export.py`) —
für den Steuerberater oder einen Wechsel. Zugangsdaten sind nicht enthalten.
DSGVO-Auskunft und Anonymisierung eines Kontakts startest du beim Kontakt.

## 10. PDF-Layout

Unter **Einstellungen → Rechnungs-Layout** wählst du **Standard, Modern,
Klassisch** oder **Minimal**, mit Vorschau. Die Auswahl gilt für neu erzeugte
Belege. Im §19-Modus stehen nur Vorlagen mit der Kleinunternehmer-Klausel zur
Wahl. Eigene Vorlagen legt der Betreiber als `.typ`-Datei in
`/inputs/pdf-templates/` ab.
