# Klein.Buch — Benutzerhandbuch (v0.1.0)

Klein.Buch ist eine lokale Buchhaltung für deutsche Kleinunternehmer nach §19 UStG.
Alle Daten bleiben auf deinem Rechner. Klein.Buch ist ein Werkzeug, kein
Steuerberater — die Pflichtangaben- und EÜR-Logik sollte vor dem Echtbetrieb von
einem Steuerberater gegengeprüft werden.

## 1. Erste Schritte

### 1.1 Backup-Passphrase (Onboarding)

Beim ersten Start verlangt Klein.Buch eine **Backup-Passphrase**, bevor du die erste
Rechnung schreiben kannst. Mit ihr werden alle Backups verschlüsselt
(Argon2id + AES-256-GCM).

Wichtig: Die Passphrase wird **nirgends gespeichert** — nicht in der Datenbank,
nicht in Logs. Verlierst du sie, sind die verschlüsselten Backups unbrauchbar.
Bewahre sie sicher auf (z. B. Passwort-Manager).

Bei jedem App-Start gibst du die Passphrase einmal ein, um Backups zu entsperren.

### 1.2 Stammdaten (Verkäuferprofil)

Unter **Einstellungen → Stammdaten** trägst du dein Unternehmen ein: Name,
Adresse, Steuernummer (sofern vorhanden), E-Mail, optional IBAN/BIC und Logo.

Der Schalter **Kleinunternehmer (§19)** ist standardmäßig aktiv. In diesem Modus
weist Klein.Buch keine Umsatzsteuer aus und setzt automatisch den Pflichthinweis
„Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen." auf jede Rechnung und in die
E-Rechnung. Ein Verzicht auf §19 (Wechsel zur Regelbesteuerung) ist möglich, bindet
dich aber 5 Jahre — Klein.Buch warnt dich vor dem Umschalten.

### 1.3 Mail-Account (für den Versand)

Unter **Einstellungen → SMTP / Mail** legst du ein Versand-Konto an: Bezeichnung,
Absender-Name und -E-Mail, SMTP-Host und -Port, optional Benutzername, und das
**Passwort (bzw. App-Passwort) deines Mail-Postfachs**. Das ist nicht zu
verwechseln mit der Backup-Passphrase aus §1.1 — hier geht es um die
Zugangsdaten deines E-Mail-Kontos.

- TLS-Schalter: Port **587** = STARTTLS, Port **465** = implizites TLS.
- Mit **Verbindung testen** prüfst du Host/Port (und ggf. Login), bevor du speicherst.
- Bei Postfächern mit 2-Faktor-Authentifizierung (z. B. Microsoft/Google) brauchst
  du ein **App-Passwort** statt des normalen Logins.
- Das Postfach-Passwort wird im **Schlüsselbund deines Betriebssystems** abgelegt
  (Windows Credential Manager / macOS Keychain / Linux Secret Service) — niemals in
  der Klein.Buch-Datenbank.
- Bestehende Konten kannst du **Bearbeiten** oder **Löschen** (Löschen entfernt
  auch das Passwort aus dem Schlüsselbund). Beim Bearbeiten bleibt das Passwort
  unverändert, wenn du das Passwort-Feld leer lässt.
- Mit **Test-Mail senden** (Empfänger-Feld + Button) verschickst du eine echte
  kurze Test-Mail über das gewählte Konto und prüfst so den vollständigen
  Versandweg — nicht nur die Verbindung.

> Hinweis zur Zustellung: Dass eine Mail vom Provider angenommen wird, heißt nicht,
> dass sie beim Empfänger im Posteingang landet. Große Anbieter (z. B. outlook.com)
> filtern Mails von kostenlosen Freemail-Absendern hart. Für zuverlässige
> Zustellung an Geschäftskunden versendest du am besten von einer **eigenen
> Domain** mit korrekt eingerichtetem SPF, DKIM und DMARC.

**Microsoft 365 / Exchange Online (ohne Passwort):** Statt SMTP-Passwort kannst du
ein Microsoft-Postfach per **OAuth** anbinden — Klein.Buch sendet dann über
Microsoft Graph. Du wählst beim Konto den Typ „Microsoft", trägst Tenant- und
Client-ID deiner eigenen Azure-App ein und klickst **Mit Microsoft verbinden**
(Anmeldung im Browser). Es wird **kein Passwort** gespeichert, nur ein
Aktualisierungs-Token im Schlüsselbund deines Betriebssystems. Die Einrichtungs-
Hilfe für die Azure-App steht direkt auf der Mail-Seite.

## 2. Rechnung schreiben und versenden

### 2.1 Kontakt anlegen

Unter **Kontakte → Neuer Kontakt** legst du den Empfänger an (Name, Adresse, ggf.
USt-IdNr., E-Mail). Die E-Mail wird beim Versand als Standard-Empfänger vorbelegt.

### 2.2 Rechnung erstellen

Unter **Rechnungen → Neue Rechnung** wählst du den Kontakt und erfasst die
Positionen (Beschreibung, Menge, Einheit, Einzelpreis). Im §19-Modus sind die
USt-Felder gesperrt; die Summen werden live berechnet. Speichern erzeugt einen
**Entwurf** mit bereits vergebener Rechnungsnummer (`RE-{Jahr}-{Nummer}`).

### 2.3 Festschreiben (Lock & Issue)

Auf der Detailseite kannst du **Validieren** (Pflichtangaben-Prüfung) und dann
**Lock & Issue** klicken. Dabei passiert:

1. Die XRechnung (CII) wird erzeugt und vom offiziellen KoSIT-Validator geprüft.
2. Daraus entsteht ein ZUGFeRD-PDF/A-3 (PDF mit eingebettetem XML).
3. PDF und XML werden unveränderbar archiviert (write-once, GoBD).
4. Ein automatisches Sicherungs-Backup wird ausgelöst.

Ab jetzt ist die Rechnung **festgeschrieben**: Kernfelder sind unveränderlich.
Korrekturen laufen nur über einen **Storno-Beleg** (Schaltfläche *Stornieren*),
nie über Löschen.

### 2.4 Versenden

Auf der festgeschriebenen Rechnung klickst du **Senden**:

- Konto auswählen (Standard-Konto ist vorausgewählt).
- Empfänger ist aus dem Kontakt vorbelegt, lässt sich überschreiben.
- Betreff und Text kommen aus der Vorlage `invoice-de` und sind editierbar.
- Das **ZUGFeRD-PDF wird automatisch angehängt**.

Nach erfolgreichem Versand steht die Rechnung auf **Versendet**; der Vorgang wird im
Audit-Log festgehalten (ohne Passwort).

### 2.5 Zahlung erfassen

Geht das Geld ein, erfasst du unter **+ Zahlung** den Betrag und das **Zahlungs-
datum**. Das Zahlungsdatum ist für die EÜR maßgeblich (Cash-Basis), nicht das
Rechnungsdatum. Bei Teilzahlung steht die Rechnung auf *Teilzahlung*, bei
vollständiger Zahlung auf *Bezahlt*.

### 2.6 Wiederkehrende Rechnungen (Abo-Rechnungen)

Für regelmäßig gleiche Rechnungen (z. B. monatliche Wartung) legst du eine
**Abo-Vorlage** an — Zugang über den Button **Wiederkehrende Rechnungen** auf der
Rechnungen-Seite. Eine Vorlage hält Kunde, Positionen (inkl. Paketen/Anfahrt wie
bei einer normalen Rechnung), Frequenz (monatlich / quartalsweise / halbjährlich /
jährlich) und Stichtag.

**Was am Stichtag passiert,** legst du je Vorlage fest:

- **Nur Entwurf vorbereiten** (Standard, prüfungssicher): Klein.Buch legt am
  Stichtag einen Rechnungs-*Entwurf* an und meldet ihn — Festschreiben und Versand
  machst du selbst.
- **Automatisch erstellen**: Die Rechnung wird automatisch festgeschrieben
  (Nummer, PDF, E-Rechnung, Archiv) — Versand machst du selbst.
- **Automatisch erstellen und senden**: zusätzlich Versand über dein
  Standard-Mail-Konto. Schlägt der Versand fehl (oder fehlt ein Standard-Konto),
  bekommst du einen Hinweis; die Rechnung bleibt festgeschrieben.

**Belegdatum ist immer der Erstellungstag** (rechtlich vorgegeben, §14 UStG/GoBD) —
nie rückdatiert. Den **Leistungszeitraum** (z. B. „Mai 2026") schreibt Klein.Buch
separat auf die Rechnung, wenn der Haken gesetzt ist. War die App länger zu, holt
sie verpasste Perioden beim nächsten Start nach (jede mit heutigem Belegdatum).
Mit **Pausieren** stoppst du eine Vorlage vorübergehend; fällige Vorlagen kannst du
über **fällige jetzt erstellen** auch sofort auslösen. Die Vorlage selbst ist ein
Stammdatum (editierbar/pausierbar); die *erzeugten* Rechnungen sind nach dem
Festschreiben unveränderlich.

## 3. Angebote

Angebote haben einen eigenen Belegkreis (`AN-{Jahr}-{Nummer}`) und einen eigenen
Lebenszyklus: *Entwurf → Versendet → Angenommen/Abgelehnt → Konvertiert/Storniert*.

### 3.1 Angebot anlegen

Unter **Angebote → Neues Angebot** wählst du den Kontakt und erfasst die
Positionen (wie bei der Rechnung). Im §19-Modus sind die USt-Felder gesperrt.
Speichern erzeugt einen **Entwurf** mit bereits vergebener Angebotsnummer.

### 3.2 Festschreiben

Auf der Detailseite klickst du **Validieren** und dann **Festschreiben**. Damit
wird das Angebot gesperrt (Status *Versendet*) und ist GoBD-konform unveränderlich
— Korrekturen laufen nur über **Stornieren** (kein Löschen). Angebote sind keine
E-Rechnungen, es läuft also keine KoSIT-/ZUGFeRD-Prüfung.

### 3.3 Annehmen (mit Vertrags-Upload)

Nimmt der Kunde an, klickst du **Annehmen**. Optional lädst du dabei den
**unterschriebenen Vertrag** (PDF/Bild) hoch — er wird write-once archiviert und
als Anhang mit dem Angebot verknüpft. Anhänge öffnest du über **Öffnen** /
**Im Ordner**.

### 3.4 PDF, Bundle und Versand

Auf einem festgeschriebenen Angebot stehen drei Aktionen bereit:

- **PDF anzeigen** — erzeugt (einmalig) das Angebots-PDF und öffnet es.
- **Bundle (Druck)** — fügt Angebot + AGB + Datenschutz zu **einem** PDF zusammen
  und öffnet es zum Drucken.
- **Versenden** — verschickt das **Bundle** per Mail: Angebots-PDF, AGB und
  Datenschutz als **drei separate Anhänge**. Empfänger, Betreff und Text sind wie
  beim Rechnungsversand vorbelegt und editierbar.

Wichtig: Für Bundle und Versand müssen je eine **aktive** AGB- und
Datenschutz-Version hinterlegt sein (siehe §4). Die zum Zeitpunkt der Ausgabe
aktiven Versionen werden **fest und unveränderlich** mit dem Angebot verknüpft —
auf der Detailseite siehst du unter *Verknüpfte Rechtsdokumente*, welche Fassung
(Version) an dieses Angebot ging (rechtlicher Nachweis).

### 3.5 In Rechnung umwandeln

Aus einem **angenommenen** Angebot erzeugt **In Rechnung umwandeln** eine
Rechnungs-Entwurf (Positionen übernommen, anpassbar). Die Rechnung wird danach wie
gewohnt festgeschrieben (§2.3). Das Angebot wechselt auf *Konvertiert* und verlinkt
die Rechnung.

## 4. Rechtsdokumente (AGB & Datenschutz)

Unter **Einstellungen → Rechtsdokumente** pflegst du AGB und
Datenschutzrichtlinie als **versionierte PDF-Dokumente**:

- **Neue Version hochladen**: PDF wählen, optional einen Titel vergeben, hochladen.
  Die Version ist zunächst inaktiv.
- **Aktivieren**: Genau eine Version pro Dokumenttyp ist gleichzeitig aktiv. Beim
  Aktivieren einer neuen Version wird die bisherige automatisch deaktiviert.
- Versionen sind **GoBD-konform unveränderlich** und werden **nie gelöscht** —
  alte Fassungen bleiben als Nachweis erhalten.

Beim Erzeugen eines Angebots-Bundles oder beim Versand werden die aktiven
Versionen fest mit dem Angebot verknüpft (§3.4). Bestehende Verknüpfungen bleiben
unverändert, auch wenn du später eine neue Version aktivierst.

> Juristik-Hinweis: AGB- und Datenschutz-Texte solltest du vor dem Echtbetrieb
> anwaltlich prüfen lassen. Klein.Buch liefert die Mechanik (Versionierung +
> Nachweis), nicht die Rechtsberatung.

## 5. Backup

Unter **Einstellungen → Backup** siehst du Status und Verlauf und kannst:

- ein **Backup-Ziel** wählen (z. B. ein OneDrive-Ordner) — Backups sind verschlüsselt,
  liegen also auch in der Cloud sicher;
- jederzeit ein **manuelles Backup** auslösen;
- aus einem Backup **wiederherstellen** (Restore-Assistent).

Automatische Backups laufen bei jedem Festschreiben/Storno und einmal täglich beim
Start. Vor jeder Wiederherstellung erstellt Klein.Buch zunächst ein
**Pre-Restore-Backup** und tauscht Datenbank und Archiv erst beim nächsten
App-Start sicher aus. Deine Vorlagen unter `inputs/` werden dabei nicht angefasst.

Den Wiederherstellungs- und Migrationsablauf für den Steuerberater beschreibt der
`docs/operations-guide.md`.

### 5.1 Zurücksetzen (alles löschen)

Unter **Einstellungen → Zurücksetzen** kannst du Klein.Buch **vollständig auf
diesem PC löschen** und auf den Anfang zurücksetzen — etwa um das Gerät
weiterzugeben oder ganz neu zu beginnen. Das ist eine **alles-oder-nichts**-Funktion;
einzelne Belege lassen sich bewusst nicht löschen (Storno statt Löschung).

Wichtig: Wegen der **10-jährigen Aufbewahrungspflicht** (GoBD/§147 AO) bietet
Klein.Buch zuerst einen **Daten-Export** an. Bestehen festgeschriebene Belege,
musst du sie exportieren **oder** ausdrücklich bestätigen, dass du deine
Aufbewahrungspflicht erfüllt hast. Danach tippst du `LÖSCHEN`, gibst dein
Daten-Passwort ein und bestätigst final.

Gelöscht wird **nur lokal**: Datenbank, Beleg-Archiv, lokale Backups sowie
gespeicherte Mail-/Backup-Zugangsdaten. Ein **externes Backup-Ziel** (Cloud-Ordner
oder SFTP-Server) bleibt bestehen — entferne es bei Bedarf selbst. Nach dem
Zurücksetzen startet die App wieder mit der Passwort-Einrichtung wie beim ersten
Mal.

## 6. Kosten erfassen

Unter **Kosten → Neue Kosten** erfasst du Betriebsausgaben: Lieferant, Datum,
Kategorie, Beschreibung und Betrag (brutto). Optional lädst du den **Beleg**
(PDF/Bild) hoch — er wird unveränderbar archiviert. Als Kleinunternehmer ziehst du
keine Vorsteuer; die Kosten gehen daher **brutto** in die EÜR. Für Leistungen, bei
denen du als Empfänger die Steuer schuldest (**§13b Reverse-Charge**), setzt du
den entsprechenden Schalter.

Das **Zahlungsdatum** ist für die EÜR maßgeblich (Cash-Basis): ohne Zahlungsdatum
zählt die Kostenposition noch nicht. Erfasste Kosten sind sofort festgeschrieben;
Korrekturen laufen über **Stornieren**, nicht über Löschen.

**Wiederkehrende Kosten** (Abos, Mieten) legst du unter **Kosten → Abos** an:
Intervall + Vorlage. Klein.Buch erzeugt die fälligen Posten automatisch als
Entwurf (Zahlung bestätigst du später). War die App länger zu, werden verpasste
Perioden beim nächsten Start nachgeholt.

**Privatentnahmen/-einlagen** (unter **Privat-Geld**) dokumentierst du der
Vollständigkeit halber — sie sind **EÜR-neutral** und beeinflussen den Gewinn
nicht.

## 7. E-Rechnungen empfangen

Seit 2025 müssen Unternehmen strukturierte E-Rechnungen annehmen können. Unter
**Kosten → E-Rechnung importieren** lädst du eine empfangene **XRechnung**
(XML, beide Formate CII/UBL) oder **ZUGFeRD** (PDF mit eingebettetem XML) hoch.
Klein.Buch liest die Daten aus, prüft sie mit dem KoSIT-Validator (**Hinweis, kein
Hindernis** — eine rechtlich empfangene Rechnung kannst du immer verbuchen) und
legt eine **Kostenposition** an. Die Originaldatei wird unveränderbar als Beleg
archiviert. Kategorie und Zahlungsdatum ergänzt du anschließend.

### 7.1 Rechnungs-Eingang einrichten (Post-v1.0, ADR 0037)

UI-Label „Rechnungs-Eingang" (Settings-Menü), Code-Identifier intern
`drop_folder_*` (Module, Settings-Keys, Route, Rule-IDs) — die folgende
Beschreibung verwendet das UI-Label. Wer regelmäßig E-Rechnungen per
E-Mail bekommt, will sie nicht jedes Mal von Hand durch den Datei-Dialog
ziehen. Unter **Einstellungen → Rechnungs-Eingang** aktivierst du einen
überwachten Ordner: Toggle „Rechnungs-Eingang aktivieren" an, Pfad
auswählen, „Speichern". Klein.Buch prüft den Ordner ab dann alle fünf
Minuten und beim App-Start; zusätzlich gibt es auf der Settings-Seite
einen Button **„Jetzt prüfen"** für Eilfälle.

- **Workflow:** XML- oder ZUGFeRD-PDF-Datei aus dem Mail-Anhang in den
  Drop-Folder kopieren (oder den Cloud-Sync-Client das machen lassen). Beim
  nächsten Tick wandert die Datei automatisch durch dieselbe Empfangs-Pipeline
  wie der manuelle Import: Parser → KoSIT-Hinweis → write-once-Archiv →
  Kostenposition (paid_date noch leer).
- **`processed/YYYY-MM/`-Routing:** erfolgreich importierte Dateien werden in
  einen Monats-Sub-Ordner verschoben, damit der Drop-Folder nicht zuwächst.
  Der Monat ist das Sync-Datum, nicht das Beleg-Datum.
- **`failed/`-Routing:** fehlgeschlagene Dateien (kaputtes XML, unsupported
  Profile, unbekannte Endung) wandern in `failed/` und bleiben dort liegen.
  Kein Auto-Delete — Klein.Buch löscht nichts im Drop-Folder. Du siehst
  fehlerhafte Dateien sofort und kannst entscheiden, ob der Absender eine
  neue Datei liefern muss oder ob du die Position manuell erfasst.
- **Polling statt Live-Watcher:** Klein.Buch fragt den Ordner nicht über
  Filesystem-Events ab (die OneDrive/iCloud-Sync-Clients liefern Events
  unzuverlässig oder gar nicht). Stattdessen liest die App den Ordner direkt
  über `read_dir` — das ist robust gegenüber Sync-Verzögerungen.
- **Notifications:** Erfolg geht in die Inbox, Fehler zusätzlich als
  OS-Toast. Beides ist in den Hinweis-Regeln schaltbar.
- **Bewusste Limit:** der Drop-Folder ist nicht rekursiv, d. h. Klein.Buch
  prüft nur Top-Level-Dateien. Unterordner-Strukturen baust du dir selbst,
  Klein.Buch greift nicht hinein.

### 7.2 Roh-XML einer empfangenen Rechnung anzeigen (Post-v1.0, ADR 0037)

Auf der Detailseite einer empfangenen E-Rechnung gibt es den Button
**„Roh-XML anzeigen"**. Klick öffnet einen Modal-Dialog mit der ursprünglichen
XML-Datei aus dem Archiv: bei ZUGFeRD wird das XML aus dem PDF/A-3-Anhang
extrahiert, bei XRechnung direkt gelesen. Der Dialog zeigt die XML
formatiert (Pretty-Print), den SHA-256-Hash und die Byte-Größe; ein
Copy-Button kopiert den vollständigen XML-Text in die Zwischenablage.

Zweck dieser Funktion ist die schnelle Prüfung durch den Steuerberater oder
den Betriebsprüfer — typische Fragen sind „Welche BT-Felder stehen drin?",
„Ist die §13b-Markierung gesetzt?", „Welcher Buyer-Snapshot ist in der
XML?". Das Anzeigen verändert nichts am Beleg und erzeugt keinen
Audit-Eintrag (Lesepfad, kein Beleg-Akt); Tamper-Detection läuft trotzdem
im Hintergrund, ein Hash-Mismatch würde sofort als `archive.tamper`-Audit
landen.

### 7.3 ZUGFeRD-Profile-Whitelist (Post-v1.0, ADR 0037)

Klein.Buch akzeptiert beim Empfang nur die ZUGFeRD-Profile, die seit der
E-Rechnungs-Pflicht 2025 gültige E-Rechnungen abbilden:

- **akzeptiert:** `EN16931`, `EXTENDED`, `XRECHNUNG`
- **abgelehnt:** `MINIMUM`, `BASIC-WL`

Beide abgelehnten Profile tragen nicht die vollständigen § 14-UStG-Pflicht-
angaben und gelten seit 2025 nur noch als buchhalterische Beilage zu einer
Papier- oder PDF-Rechnung. Klein.Buch zeigt in dem Fall eine klare
Fehlermeldung mit dem erkannten Profil-Namen. Tipp: Wenn ein Lieferant nur
MIN- oder BASIC-WL-Anhänge schickt, ist das ein guter Anlass, ihn auf
EN16931 oder XRECHNUNG umzustellen — andernfalls erfasst du die Position
manuell und bittest um einen gültigen Beleg.

## 8. Anlagen & Abschreibung (AfA)

Größere Anschaffungen (über der GWG-Grenze) schreibst du über mehrere Jahre ab.
Unter **Anschaffungen → Neue Anschaffung** erfasst du Bezeichnung, Datum,
Anschaffungskosten, Methode (linear / GWG-Sofort / Computer-Sonderregel 1 Jahr)
und ggf. den **betrieblichen Anteil**. Mit **AfA buchen** erzeugt Klein.Buch die
Abschreibung für das Geschäftsjahr (der Lauf ist wiederholbar/idempotent). Verkaufst
du eine Anlage, erfasst du die **Veräußerung** (Erlös) — Restbuchwert und
Gewinn/Verlust fließen automatisch in die EÜR. Die AfA-Sätze stammen aus der
BMF-Tabelle unter `inputs/afa-tabellen.json`, die du bei Bedarf pflegen kannst.

## 9. EÜR & Steuer-Export

Unter **Steuer (EÜR)** wählst du das Jahr und siehst die Einnahmen-Überschuss-
Rechnung auf **Cash-Basis** (maßgeblich ist der Zahlungszeitpunkt): Betriebs-
einnahmen, Betriebsausgaben nach Kategorie, AfA, Anlagen-Veräußerungen und der
Überschuss/Verlust. Storno-Erstattungen erscheinen als negative Einnahme im Jahr
des Storno-Belegs.

Unter **Steuer (EÜR) → Exportieren** erzeugst du:

- **Für die eigene Abgabe (ELSTER):** eine Ausfüllhilfe (welcher Betrag in welche
  Zeile der Anlage EÜR gehört) plus ein PDF „Anlage EÜR".
- **Für den Steuerberater:** einen **DATEV-Buchungsstapel** (Kontenrahmen SKR03
  voreingestellt, SKR04 umschaltbar) und ein **Steuerberater-Paket (ZIP)** mit
  Deckblatt, EÜR-PDF, DATEV-Datei, Einzelaufstellungen und Stammdaten.

Sind für das Jahr noch nicht alle Abschreibungen gebucht, weist dich ein Banner
darauf hin und bietet **AfA jetzt buchen** an — sonst fehlte die AfA im Export.
Die Konten-Zuordnung im DATEV-Export ist ein **Vorschlag** und vom Steuerberater
zu prüfen.

## 10. Hinweise & Geschäftsjahr abschließen

Die **Hinweise** (Glocke oben) erinnern dich an Fälliges: Belege erfassen,
überfällige Rechnungen, Backup überfällig, Geschäftsjahr abschließen. Welche
Erinnerungen aktiv sind, steuerst du unter **Hinweise → Regeln**.

Unter **Geschäftsjahr** schließt du ein **abgelaufenes** Jahr ab. Dabei wird die
AfA gebucht, das Jahr **prüfungssicher festgeschrieben** (danach unveränderlich,
§146 AO) und ein Sicherungs-Backup erstellt. Der Abschluss ist **unumkehrbar** und
verlangt eine entsperrte Backup-Passphrase. **Korrekturen** an einem
abgeschlossenen Jahr sind weiterhin über einen **Storno-Beleg** möglich (er wirkt
im Storno-Jahr). Die automatische AfA-Buchung zum Jahreswechsel ist standardmäßig
aktiv und unter **Geschäftsjahr** abschaltbar.

## 11. PDF-Layout wählen

Unter **Einstellungen → Rechnungs-Layout** wählst du, wie deine Rechnungen und
Angebote als PDF aussehen: **Standard, Modern, Klassisch** oder **Minimal**. Mit
**Vorschau** öffnest du je Vorlage ein Muster-PDF. Die Auswahl gilt für **neu
erzeugte** Belege; bereits ausgestellte bleiben unverändert. Im §19-Modus stehen
nur Vorlagen mit der Kleinunternehmer-Klausel zur Wahl. Eigene Vorlagen kannst du
als `.typ`-Datei in `inputs/pdf-templates/` ablegen.
