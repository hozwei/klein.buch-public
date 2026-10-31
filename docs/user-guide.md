# Klein.Buch — Benutzerhandbuch (Kurzfassung)

Das vollständige Handbuch ist in Klein.Buch eingebaut: in der Seitenleiste unter
**Hilfe**, mit Volltext-Suche und Glossar; F1 öffnet das Kapitel zur gerade
offenen Seite. Die Kapitel liegen auch im
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
Logo und Unterschrift unter **Einstellungen → Rechnungs-Layout**. Telefon und
Steuernummer bzw. USt-IdNr. sind als „Pflicht für die E-Rechnung“ gekennzeichnet:
Speichern geht auch ohne sie (mit Warnung), ausstellen lässt sich eine Rechnung
erst mit beiden. Solange eine dieser Angaben fehlt, erinnert die Startseite mit
einem Hinweis und der ersten Aufgabe „Firmendaten ergänzen“.

Der Schalter **Kleinunternehmer (§19)** ist standardmäßig aktiv. In diesem Modus
weist Klein.Buch keine Umsatzsteuer aus und setzt automatisch den Pflichthinweis
„Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen." auf jede Rechnung und in die
E-Rechnung. Der Wechsel zur Regelbesteuerung fragt vorher nach und erklärt beide
Fälle: Ein freiwilliger Verzicht auf §19 bindet dich 5 Jahre, der Wegfall durch
die 100.000-€-Grenze bindet nicht.

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
  Mail-Ports, höchstens zehn Tests je Minute). Gespeichert werden nur diese Ports;
  einen anderen (etwa 1025 eines internen Relays) gibt der Betreiber über
  `KLEIN_BUCH_ALLOWED_TEST_PORTS` frei.
- **Microsoft 365** wird über SMTP angebunden (z. B. `smtp.office365.com`, Port
  587, sofern der Anbieter SMTP-Versand erlaubt). Die frühere Microsoft-Anmeldung
  über OAuth gibt es im Server nicht.
- Jeder Versand steht im **E-Mail-Protokoll** (**Einstellungen →
  E-Mail-Protokoll**), auch Fehlschläge. Kann ein Eintrag dort ausnahmsweise
  nicht geschrieben werden, meldet Klein.Buch das nach dem Senden und unter
  **Hinweise** („E-Mail-Protokoll unvollständig“); der Versand steht dann im
  Audit-Log.

> Hinweis zur Zustellung: Dass eine Mail vom Provider angenommen wird, heißt
> nicht, dass sie beim Empfänger im Posteingang landet. Für zuverlässige
> Zustellung an Geschäftskunden versendest du am besten von einer **eigenen
> Domain** mit korrekt eingerichtetem SPF, DKIM und DMARC.

### 1.4 Bedienung

Auf dem Telefon, auf Tablets und in schmalen Fenstern liegt die Navigation
hinter dem Menü-Knopf (drei Striche) in der Kopfleiste. Jede Seite zeigt oben den
nächsten Schritt als Hauptaktion; seltene und gefährliche Aktionen stehen im Menü
**Mehr** (auf dem Telefon auch die Nebenaktionen). Beträge gibst du deutsch ein:
`1.234,56` (Komma vor den Cent, Punkt nur als Tausendertrenner, kein Minus); nach
dem Verlassen des Felds zeigt Klein.Buch, wie es die Eingabe gelesen hat. Fehler
stehen direkt am Feld, der Fokus springt zum ersten. Meldungen erscheinen unten;
Fehler bleiben stehen, bis du sie schließt. Was sich zurücknehmen lässt
(archivieren, deaktivieren, pausieren), geschieht ohne Rückfrage und bietet
„Rückgängig“ an; Festschreiben, Stornieren und Wiederherstellen fragen vorher
nach. Geschützte Aktionen bestätigst du im Dialog „Aktion bestätigen: …“ mit
Daten-Passwort und App-Code; „Abbrechen“ ändert nichts. Die Oberfläche orientiert
sich an WCAG 2.2 AA (geprüft mit axe-core und manuell); die Tastenbelegung (Tab,
Escape, F1) steht im Handbuch-Kapitel „Willkommen in Klein.Buch“.

## 2. Rechnungen

- **Kontakt anlegen** unter **Kontakte**; die E-Mail wird beim Versand
  vorbelegt. In der Rechnung wählst du den Kontakt aus einer Liste, ab acht
  Kontakten mit Suchfeld.
- **Rechnung anlegen** unter **Rechnungen → + Neue Rechnung**: Kontakt,
  Positionen (auch Pakete und Anfahrt; Einheiten im Klartext wie Stunde, Stück,
  Pauschale), Fälligkeit (Pflicht), Währung Euro. Unter **Leistung** wählst du
  ein Datum oder einen Zeitraum (von–bis); das Leistungsdatum folgt dem
  Rechnungsdatum, bis du es änderst. Beides darf nach dem Rechnungsdatum liegen
  (Vorausrechnung, Hinweis „Rechnung vor der Leistung“). Bei Rechnungen an
  Behörden trägst du die Leitweg-ID schon im Entwurf ein. Der Einzelpreis neuer
  Positionen startet leer. Im §19-Modus sind die USt-Felder gesperrt. Gespeichert wird ein
  **Entwurf** mit bereits vergebener Nummer (`RE-{Jahr}-{Nummer}`); das
  Geschäftsjahr ergibt sich aus dem Rechnungsdatum. Ändern geht über
  **Bearbeiten** (Seite „Entwurf bearbeiten“), ein nicht mehr gebrauchter Entwurf
  wird über **Mehr → Entwurf verwerfen** mit Grund verworfen; die Nummer bleibt
  verbraucht.
- **Rechnung ausstellen**: Unter den Positionen des Entwurfs prüft die Karte
  **Ausstellen** beim Öffnen, ob alles da ist, und nennt fehlende Angaben mit
  Links zur Korrektur („Firmendaten ergänzen“, „Kontakt ergänzen“, „Entwurf
  bearbeiten“). Die Leitweg-ID steht dort vom Entwurf vorbelegt; ein geänderter
  Wert gilt für diese E-Rechnung (sonst leer lassen). Klein.Buch erzeugt die XRechnung, lässt sie vom
  KoSIT-Validator prüfen, erzeugt das ZUGFeRD-PDF/A-3 (Profil XRECHNUNG, passend
  zur eingebetteten XRechnung 3.0), archiviert beides unveränderbar und legt ein
  Backup an. Ab jetzt ist die Rechnung **festgeschrieben**; Korrekturen laufen
  nur über **Mehr → Stornieren** mit Rückfrage (Storno-Beleg
  `ST-{Jahr}-{Nummer}`), nie über Löschen.
- **PDF öffnen** zeigt das PDF im Browser (auf dem Telefon in der PDF-Ansicht des
  Browsers), **Mehr → PDF herunterladen** speichert es.
- **Senden** ist nach dem Ausstellen der nächste Schritt und verschickt das
  ZUGFeRD-PDF über dein Postfach; Empfänger, Betreff und Text sind vorbelegt und
  änderbar. Gehst du an eine andere als die beim Beleg hinterlegte Adresse, fragt
  Klein.Buch einmal Daten-Passwort und App-Code ab (Schutz, falls jemand deine
  Sitzung übernommen hat); Betreff und Text änderst du ohne Rückfrage. Das gilt
  auch beim Angebotsversand.
- **+ Zahlung** erfasst Zahlungseingänge mit Datum und Konto; für die EÜR zählt
  das Zahlungsdatum (Cash-Basis), Teilzahlungen auch über Jahresgrenzen. Status
  in Liste und Detail: Entwurf, Ausgestellt, Versendet, Teilbezahlt, Bezahlt,
  Überfällig (Fälligkeit vorbei, Betrag offen), Storniert, Verworfen; ein
  Storno-Beleg trägt statt eines Zahlstatus „Storno-Beleg“.
- **Abo-Rechnungen** (Nebenaktion auf der Rechnungen-Seite): Abo-Vorlagen mit
  Frequenz und Stichtag; je Vorlage „Nur Entwurf“, „Automatisch ausstellen“ oder
  „Ausstellen + senden“. „Jetzt erstellen“ fragt bei den beiden automatischen
  Modi vorher nach; Pausieren geht ohne Rückfrage und lässt sich rückgängig
  machen. Das Belegdatum ist immer der Erstellungstag. Bis drei verpasste
  Perioden holt Klein.Buch selbst nach (ältere als Entwurf, nie automatisch
  versendet), bei mehr fragt es über „Rückstand …“ nach.

## 3. Angebote

Eigener Belegkreis (`AN-{Jahr}-{Nummer}`) und Lebenszyklus: *Entwurf →
Fertiggestellt → Angenommen/Abgelehnt → Umgewandelt*. **Fertigstellen** schreibt
das Angebot fest, verschickt es aber nicht; danach ist **Senden** der nächste
Schritt, und die Detailseite zeigt „versendet am …“ aus dem E-Mail-Protokoll.
Ein Entwurf wird **verworfen**, ein fertiggestelltes Angebot **storniert**.
Festgeschriebene Angebote sind unveränderlich. **PDF öffnen** zeigt das Angebot,
**Mehr → Druck-PDF (mit AGB)** fügt Angebot, AGB und Datenschutz zu einem PDF
zusammen; der Versand schickt sie als drei Anhänge. Beim **Annehmen** kannst du
den unterschriebenen Vertrag hochladen; er wird unveränderbar archiviert. **In
Rechnung umwandeln** erzeugt aus einem angenommenen Angebot einen
Rechnungs-Entwurf.

AGB und Datenschutz pflegst du unter **Einstellungen → AGB & Datenschutz** als
versionierte PDF-Dokumente; je Typ ist genau eine Version aktiv, alte Fassungen
bleiben als Nachweis erhalten und werden nie gelöscht.

## 4. Kosten, E-Rechnungen, Privatbewegungen

- **+ Neue Kosten**: zuerst der Beleg als PDF oder Bild (auf Touch-Geräten auch
  direkt mit der Kamera über „Foto aufnehmen“), dann Lieferant, Datum,
  Kategorie, Beschreibung und Betrag (brutto). Kosten sind ab dem Speichern
  festgeschrieben, deshalb heißt der Knopf **Kosten festschreiben** und fragt mit
  den Eckwerten nach. Korrektur über **Mehr → Stornieren**; danach legt
  **Korrigiert neu erfassen** neue Kosten mit den Werten des Originals an, und
  **Als Vorlage für neue Kosten** übernimmt die Werte einer bestehenden Position
  mit dem heutigen Datum. Für die EÜR zählt das Zahlungsdatum. Bei §13b sind
  USt-Satz und -Betrag gesperrt (0).
- **E-Rechnung importieren**: empfangene XRechnung (XML, CII oder UBL) oder
  ZUGFeRD-PDF hochladen und **Einlesen und prüfen**. Klein.Buch liest die Daten,
  prüft sie mit dem KoSIT-Validator (Hinweis, kein Hindernis), archiviert das
  Original und legt eine Kostenposition an. Angenommen werden die ZUGFeRD-Profile
  `EN16931`, `EXTENDED` und `XRECHNUNG`; `MINIMUM` und `BASIC-WL` sind keine
  gültigen E-Rechnungen und werden abgelehnt. Enthält die Rechnung die
  Steuerkategorie K (innergemeinschaftliche Lieferung aus dem EU-Ausland), weist
  die Vorschau darauf hin: Erwerbsteuer kann auch mit §19 anfallen; das prüfst du
  selbst, Klein.Buch erfasst nur den Betrag laut Rechnung. Gutschriften (z. B. Typ 381)
  importiert Klein.Buch nicht automatisch — sie erfasst du als Korrektur;
  abgeschnittene oder nicht UTF-8-codierte Dateien werden mit Meldung abgelehnt.
  Die Roh-XML einer empfangenen Rechnung zeigt die Detailseite.
- **Kosten-Abos** für regelmäßige Kosten (Miete,
  Software): automatisch buchen oder nur am Stichtag erinnern (Hinweis „Abo
  fällig“); verpasste Termine wie bei den Abo-Rechnungen (bis drei selbst, mehr
  nach Rückfrage).
- **Privatbewegungen** dokumentiert Entnahmen und Einlagen (EÜR-neutral);
  **Privatbewegung festschreiben** fragt vorher nach, jede Bewegung hat eine
  eigene Detailseite.

## 5. Anschaffungen und Abschreibung

Unter **Anschaffungen** erfasst du Wirtschaftsgüter des Anlagevermögens mit
Methode und betrieblichem Anteil (Zahl von 0 bis 100 %); werden sie aus Kosten
übernommen, stehen die Anschaffungskosten fest (brutto bei §19, netto bei
Regelbesteuerung). **Abschreibung buchen** zeigt die AfA je Jahr und bucht sie
(wiederholbar). Der **Abgang** (Verkauf, Verschrottung, Privatentnahme oder
Geschenk) zeigt vor dem Buchen den Restbuchwert zum gewählten Datum; ist der
Abgang zu diesem Datum nicht möglich, steht der Grund am Datumsfeld. Die selbst
geschuldete §13b-Steuer zu einer Anschaffung erfasst du über „Steuer als Kosten
erfassen“ auf der Anschaffung und übernimmst sie mit „Als Anlage übernehmen“; die
Auswahl „Selbst geschuldete Steuer (§13b) zu Anschaffung“ belegt dabei Datum,
Methode, Nutzungsdauer, Kategorie und Anteil vor. Die AfA-Sätze
kommen aus der Tabelle `afa-tabellen.json` im Volume `/inputs`.

## 6. EÜR und Steuer-Export

Unter **Steuer (EÜR)** siehst du die Einnahmen-Überschuss-Rechnung eines Jahres
auf Cash-Basis, mit einem Hinweis, wenn in dem Jahr die Regelbesteuerung galt,
und einer Warnung, wenn ein abgeschlossenes Jahr vom festgeschriebenen Stand
abweicht. **Für die Steuererklärung exportieren** erzeugt die
ELSTER-Ausfüllhilfe mit „Anlage EÜR"-PDF, den DATEV-Buchungsstapel (SKR03,
umschaltbar auf SKR04; Berater- und Mandantennummer optional, beides merkt sich
dein Browser) und das Steuerberater-Paket (ZIP). Alle Exporte kommen als
Download in den Browser; jede Datei lässt sich nur einmal abrufen. Die
Konten-Zuordnung im DATEV-Export ist ein Vorschlag und vom Steuerberater zu
prüfen.

## 7. Geschäftsjahr und Hinweise

Unter **Geschäftsjahr** schließt du ein **abgelaufenes** Jahr ab: AfA buchen,
Jahr prüfungssicher festschreiben (§146 AO), Backup. Der Abschluss ist
unumkehrbar; Korrekturen danach nur per Storno-Beleg. Er wartet auf gerade
laufende Buchungen; offene Entwürfe des Jahres blockieren ihn nicht, lassen sich
danach aber nur noch verwerfen (der Abschluss-Dialog warnt davor). Abgeschlossene
Jahre stehen im **Abschlussprotokoll**. Weicht die EÜR eines abgeschlossenen
Jahres später vom festgeschriebenen Stand ab, meldet Klein.Buch das als Hinweis
und in jedem Export (Handbuch „Geschäftsjahr-Abschluss“). Die automatische AfA
zum Jahreswechsel ist standardmäßig aktiv und dort abschaltbar.

**Hinweise** erinnern an Fälliges (überfällige Rechnungen, Backups, fehlende oder
gescheiterte externe Backup-Ziele, Geschäftsjahr abschließen) und — wenn unter
**Einstellungen → Sicherheit & Anmeldung** eingeschaltet — an neue Versionen.
Erledigtes hakst du ab („Alle abhaken“ fragt nur nach, wenn dringende Hinweise
dabei sind); die Zahl offener Hinweise steht in der Seitenleiste. Welche
Erinnerungen aktiv sind, steuerst du unter **Hinweise → Erinnerungen einstellen**
(im Einstellungen-Hub „Erinnerungen einstellen“).

## 8. Backups und Wiederherstellen

Unter **Einstellungen → Backups**:

- **Status** mit dem Alter der letzten externen Kopie, **Backup-Verlauf**;
  **Jetzt sichern** zeigt danach das Ergebnis je Ziel („ausgesetzt“ nach einer
  Wiederherstellung bis zur Freigabe ist kein Fehler). Das Alter der externen
  Kopie steht auch auf der Startseite.
- **Externe Ziele**: Ordner unter `/backup-targets` (vom Betreiber
  eingebunden, z. B. NAS oder Sync-Ordner) und SFTP-Server, gern mehrere.
  Backups sind verschlüsselt und liegen auch bei einem Anbieter sicher. Ohne
  externes Ziel liegen alle Backups auf demselben Server wie die Daten.
- **Wiederherstellen**: Backup-Datei hochladen oder ein lokales Backup wählen und
  das Daten-Passwort dieses Backups eingeben. Die Rückfrage nennt den ganzen
  Ablauf: Klein.Buch sichert vorher den aktuellen Stand, fragt Daten-Passwort und
  App-Code ab und startet danach neu; anschließend meldest du dich mit dem
  Daten-Passwort des Backups an.

Automatische Backups entstehen bei jedem Festschreiben, einmal je Kalendertag
zur eingestellten Uhrzeit (Standard 02:30), vor jeder Wiederherstellung und vor
jeder Datenbank-Umstellung durch ein Update. Nach einer Wiederherstellung bleiben
Abo-Läufe und Versand angehalten und Backups gehen nur in den lokalen Ordner,
bis du im Banner mit „Prüfen und freigeben“ freigibst; bereits vergebene
Belegnummern werden nicht erneut vergeben (Einstellungen → Backups →
Nummernkreise).
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
`/inputs/pdf-templates/` ab; stellt eine Vorlage keine Stornos dar (Kennzeichen
„Ohne Storno-Darstellung“), entstehen Storno-Belege mit der dort genannten
mitgelieferten Vorlage.
