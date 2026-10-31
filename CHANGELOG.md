# Changelog

Folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/). Versionen nach CalVer
`JJJJ.M.PATCH` (Monats-Release, PATCH für Korrekturen im selben Monat).

## [2026.11.1] — Fachliche Korrekturen und DATEV-Format, in Arbeit

Korrekturen nach 2026.11.0: Steuerstatus nach § 19 UStG mit Wirkungsdatum und Grund,
Leistungszeitraum und Vorausrechnung, Korrektur falsch gebuchter Zahlungen per Gegenbuchung,
bearbeitbare Angebots-Entwürfe, DATEV-Buchungsstapel nach der Formatbeschreibung sowie die
offenen Punkte aus Lauf 2026.11.

- **Spezifikationen berichtigt** (`inputs/specs/`): § 19 UStG in der Fassung ab 2025 (Grenzen,
  Gesamtumsatz nach vereinnahmten Entgelten, Gründungsjahr, Verzicht nach Abs. 3 mit fünf
  Kalenderjahren ab dem Wirkungsjahr, unrichtiger Steuerausweis nach § 14c Abs. 1), § 13b UStG
  (auch Kleinunternehmer schulden die Steuer), Pflichtangaben nach § 14 UStG und § 34a UStDV,
  DATEV-Format (Präfix `EXTF_`, Kopf Version 700, Pflichtnummern).
- **Steuerstatus nach § 19 UStG mit Wirkungsdatum und Grund** (ADR 0042 Nr. 1–6): Statt des
  Schalters unter „Meine Firmendaten“ führt Klein.Buch eine unveränderliche Historie
  (Verzicht, Wegfall wegen des Vorjahres, Wegfall im laufenden Jahr bzw. im Gründungsjahr,
  Rückkehr; ein Irrtum wird mit Grund widerrufen). Die Rückkehr ist nach einem gesetzlichen
  Wegfall ohne Sperre möglich, nach einem Verzicht nach fünf Kalenderjahren einschließlich des
  Wirkungsjahres (bisher ein Jahr zu lang). Ein Wechsel mitten im Jahr wirkt ab dem Umsatz,
  der die Grenze überschreitet. Jeder Beleg nimmt den Status an seinem Leistungsdatum (sonst
  Rechnungsdatum), Kosten am Belegdatum; EÜR, ELSTER-Ausfüllhilfe und DATEV-Stapel teilen ein
  Wechseljahr. Klein.Buch zeigt den Gesamtumsatz nach vereinnahmten Entgelten (Vorjahr und
  laufendes Jahr) und meldet erreichte Grenzen; einen rückwirkenden Wechsel nimmt es nur in
  offenen Geschäftsjahren an und nennt betroffene festgeschriebene Rechnungen.
- **Zahlungsbuch und „Zahlung stornieren“** (ADR 0042 Nr. 7–9): Zahlungen auf Rechnungen stehen
  in einem unveränderlichen Zahlungsbuch; Betrag, Datum, Status und Historie einer Rechnung sind
  daran gebunden (bisher ließ sich die Zahlungshistorie einer festgeschriebenen Rechnung
  ändern). Eine falsch erfasste Zahlung wird mit Grund durch eine Gegenbuchung storniert, die am
  ursprünglichen Zahldatum wirkt (§ 11 EStG), danach wird richtig neu erfasst; im
  abgeschlossenen Geschäftsjahr lehnt Klein.Buch ab und verweist auf § 153 AO. EÜR,
  ELSTER-Ausfüllhilfe, DATEV-Stapel, Steuerberater-Paket und DSGVO-Auskunft zeigen
  Gegenbuchungen.
- **Leistungszeitraum, Vorausrechnung, Leitweg-ID am Entwurf** (ADR 0042 Nr. 10–12, 22–24):
  Rechnungen tragen wahlweise ein Leistungsdatum oder einen Leistungszeitraum (XRechnung BG-14,
  alle vier PDF-Vorlagen); das Leistungsdatum darf nach dem Rechnungsdatum liegen (Rechnung vor
  der Leistung). Abo-Rechnungen setzen den tatsächlichen Abrechnungszeitraum, auch bei
  quartalsweiser, halbjährlicher und jährlicher Abrechnung; ein Storno übernimmt den Zeitraum.
  Die Leitweg-ID wird schon am Entwurf (und an der Abo-Vorlage) gespeichert.

### Hinweise für Betreiber (Update von 2026.11.0)

- **Datenbank-Migration:** Beim ersten Entsperren hebt Klein.Buch das Schema an (Migrationen
  ab 0033) und legt vorher das übliche Pflicht-Backup an. Ein Image 2026.11.0 kann die
  migrierte Datenbank und danach angelegte Backups nicht öffnen; zurück geht es nur über das
  Pflicht-Backup.
- **Steuerstatus:** Der bisherige Stand erscheint als „Wechsel aus dem bisherigen Schalter
  übernommen“ (Bindung nach § 19 Abs. 3 UStG). War der Wechsel in Wahrheit ein gesetzlicher
  Wegfall, den Eintrag widerrufen und mit dem richtigen Grund neu eintragen.
- **Herunterfahren unverändert:** `stop_grace_period` mindestens `90s`; das Herunterfahren
  dauert bis zu 75 s, zusammen höchstens 75 s.

## [2026.11.0] — Oberfläche für Smartphone, Tablet und Desktop, veröffentlicht 2026-10-27

Die Oberfläche für Smartphone, Tablet und Desktop (Lauf 2026.11), dazu die Umsetzung der
Entscheidungen nach v2026.10.0 (Lauf E) sowie Funktion und Technik für den Dauerbetrieb
(Lauf F).

### Lauf 2026.11 — Oberfläche (UI/UX)

- Version 2026.11.0 (ADR 0041 Nr. 1).
- Neue Entscheidungsgrundlage ADR 0041 „UI/UX-Standards (2026.11)“ und Designleitlinie
  `docs/architecture/design.md`: Die Oberfläche orientiert sich an WCAG 2.2 AA und wird mit
  axe-core und eigenen Messungen auf Smartphone, Tablet und Desktop geprüft.
- **Beträge und Mengen deutsch eingeben:** Alle Betrags- und Mengenfelder nehmen „1.234,56“ an
  (Leseregel in ADR 0041 Nr. 15), zeigen nach dem Verlassen den erkannten Wert im deutschen Format
  und melden zu viele Nachkommastellen oder ein Minuszeichen am Feld, statt still zu runden. Kein
  Scrollrad-Effekt mehr. Beträge erscheinen mit geschütztem Leerzeichen vor „€“, Prozent als
  „19 %“, IBAN in Vierergruppen.
- **Fehler am Feld:** Pflicht- und Formatfehler stehen direkt am Feld (für Screenreader verknüpft),
  der Fokus springt zum ersten fehlerhaften Feld; Server-Ablehnungen mit mehreren Gründen stehen als
  Hinweis über dem Formular statt als kurze Meldung.
- **Smartphone und Tablet:** Navigation auf Touch-Geräten immer als Schublade (auch Tablet quer),
  Tippflächen mindestens 44 × 44 px, Eingaben in 16 px (kein Auto-Zoom auf iOS), Dialoge auf kleinen
  Bildschirmen bildschirmfüllend, Tabellen bis 820 px als Karten, im Querformat in sich scrollend.
- **Schneller auf dem Handy:** Die Oberfläche lädt bei langsamer Verbindung rund doppelt so schnell (Dateien vorkomprimiert, ADR 0041 Nr. 212).
- **Lesbarkeit und Kontrast:** dunklere Hinweistexte, Feldrahmen, Checkboxen und Schalter;
  deutlicher Fokusrahmen (auch auf der Seitenleiste); Kopfleiste und Dialoge ohne Milchglas.
- **Tastatur und Screenreader:** Dialoge halten den Fokus, schließen mit Escape und geben den Fokus
  zurück; Gefahr-Dialoge starten auf „Abbrechen“; Sprunglink „Zum Inhalt“; alle Felder,
  Auswahllisten und Symbolknöpfe tragen einen Namen; die klebende Kopfleiste verdeckt den Fokus
  nicht mehr.
- **Meldungen:** erscheinen unten; Fehlermeldungen bleiben stehen, bis sie geschlossen werden,
  Erfolgsmeldungen pausieren bei Zeiger oder Fokus.
- **Anmelden und Passwort:** Knopf „Anzeigen“ an Passwortfeldern, Stärke-Hinweis bei neuer
  Passphrase (nur Hinweis), Autofill-Angaben für Passwortmanager.
- **Formulare:** Verkäuferprofil mit Autofill-Angaben; Kontakte ohne Autofill (Daten Dritter);
  Leistungsdatum mit dem Rechnungsdatum vorbelegt; PDF-Vorlage als Auswahl; betrieblicher Anteil
  einer Anlage als Zahl statt Schieberegler; Belegfoto: Datei-Feld mit Galerie und Dateien plus
  eigener Knopf „Foto aufnehmen“.
- **Belege:** Entwürfe heißen einheitlich „Verwerfen“; Stornos zeigen Nummer, Datum und Link des
  Originals (auch beim Versand, Titel „Stornorechnung versenden“); das Storno-Datum nennt den
  frühesten zulässigen Tag; der Versand kündigt die Bestätigung bei abweichendem Empfänger vorab an;
  Zahlungs-Konto mit Standardkennzeichnung.
- **Kosten und Import:** bei §13b sind USt-Satz und -Betrag gesperrt (0) mit Erklärung; Fehler
  „§13b mit USt“ am Feld; Hinweis, wenn das Zahldatum vor dem Belegdatum liegt.
- **EÜR, Geschäftsjahr, Anlagen:** Export-Vorschau mit deutschen Daten und dem Hinweis, dass jede
  Datei nur einmal abrufbar ist; Warnung bei Abweichung vom Jahresabschluss; Abschluss-Dialog warnt
  vor offenen Entwürfen; Anschaffungskosten aus Kosten mit Erklärung brutto/netto; Entnahmewert bei
  Privatentnahme als Pflichtfeld.
- **Einstellungen:** Übersicht in drei Gruppen mit Links zu Protokollen und Regeln; §19-Dialog
  erklärt drei Fälle — freiwilliger Verzicht (§ 19 Abs. 3 UStG, ab Jahresbeginn, mindestens
  5 Kalenderjahre gebunden) sowie den Wegfall durch die 25.000-€-Grenze im Vorjahr oder die
  100.000-€-Grenze im laufenden Jahr (ohne Bindung) — und weist auf § 15a UStG hin; „Meine
  Firmendaten“ nennt, dass Klein.Buch die Rückkehr in allen Fällen erst ab dem genannten Datum
  zulässt; Backup-Seite ohne
  Desktop-Reste, mit Alter der letzten externen Kopie; Hinweise zu SMTP-/SFTP-Host; Sitzungsliste
  mit Kurzkennung passend zum Server-Log; Audit-Protokoll mit Spalte „Ausgelöst von“ und betroffener
  Datei bei Prüfsummenfehlern.
- **Tablet:** Formulare (Kontakt, Firmendaten, SFTP-Ziel, Paket) zweispaltig, wo Platz ist;
  Listen im Hochformat in zwei Kartenspalten; im Querformat und am Desktop weichen Nebenspalten, damit
  Nummer, Partner, Betrag, Status und „Öffnen“ ohne Seitwärts-Scrollen sichtbar sind; Tabellen
  scrollen auf Touch-Geräten in sich statt die Seite zu verbreitern. Das Handbuch scrollt mit der
  Seite, Kapitelliste und Suche bleiben stehen; eine unbekannte Handbuchseite führt zurück ins
  Handbuch. Erklärungen stehen sichtbar statt nur im Tooltip.
- **Desktop:** Auf breiten Bildschirmen bleiben Listen, Formulare und Text lesbar breit; Listen
  sind dichter, die Zeile unter der Maus ist leicht getönt, ab 1280 px sind alle Spalten sichtbar.
  Das Handbuch ist eine durchgehende Seite mit stehender Kapitelliste. Beim Seitenwechsel beginnt
  die neue Seite oben. Kontakte sind nach deutscher Schreibweise sortiert; Zahlungskonten werden in
  einer beschrifteten Formularzeile bearbeitet.
- **Zoom und Tastatur:** Bei 200 % und 400 % Zoom scrollen Dialoge als Ganzes, die obere Leiste
  klebt bei geringer Höhe nicht mehr und verdeckt den Fokus nicht; Datums- und Uhrzeitfelder zeigen
  den Fokusrahmen; der Fokus bleibt im PDF-Vorschau-Dialog.
- **Drucken:** Rechnungsliste, Rechnung und EÜR drucken ohne Menü, Leisten und Knöpfe, als echte
  Tabelle ohne abgeschnittene Spalten.
- **Gestaltung:** ruhigeres, präziseres Erscheinungsbild (eine Bedienhöhe für Knöpfe und Felder,
  kleinere Radien, Haarlinien statt weicher Schatten, eigene Auswahlfelder ohne Doppelrahmen,
  Belegnummern als Monospace), Seitenleiste mit Gruppe „Sitzung“, Startseite ohne Begrüßung mit
  Datum, Kennzahlen und „Letzte Belege“ im Raster. Web-App-Manifest für „Zum Startbildschirm“
  (ohne Offline-Funktion).
- **Seitentitel und Hilfe:** Jede Seite hat einen eigenen Titel im Browser-Tab; F1 öffnet das passende
  Handbuch-Kapitel; unbekannte Adressen zeigen eine deutsche Fehlerseite mit Weg zurück.
- **Mehr-Menü:** Seiten mit vielen Aktionen zeigen den nächsten Schritt als Hauptaktion, Neben- und
  Gefahraktionen im Menü „Mehr“ (auf dem Handy auch die Nebenaktionen).
- **Belege:** eigene Seite „Entwurf bearbeiten“ (alte Links leiten weiter); Karte „Ausstellen“ unter
  den Positionen mit Prüfung beim Öffnen und verständlichen Ablehnungsgründen samt Links zur
  Korrektur; nach dem Ausstellen ist „Senden“ der nächste Schritt; Status „Überfällig“ auch in Liste
  und Detail; Angebote heißen nach dem Fertigstellen „Fertiggestellt“ statt „Versendet“; Einheiten
  im Klartext (Stunde, Stück, Pauschale …) mit Fehler am Feld bei alten Freitext-Einheiten;
  Zahlungshistorie mit Konto je Zahlung; der Bezahlt-Hinweis ist in der Rechnung sichtbar;
  Fälligkeit ist Pflicht, Währung fest Euro; Abo-Läufe mit Auto-Modus fragen vor dem Ausstellen
  und Versenden nach; Storno mit Rückfrage; Jahr-Filter als Auswahl; Kontaktauswahl mit Suche.
- **Behoben (Belege):** Eine im Rechnungs-Editor eingetragene Leitweg-ID ging verloren und die
  E-Rechnung enthielt „N/A“; die Leitweg-ID wird jetzt beim Ausstellen eingegeben. Der
  Storno-Beleg übernimmt die Leitweg-ID der Rechnung aus deren archivierter E-Rechnung (mit
  Prüfsummen-Kontrolle), damit die Behörde ihn zuordnen kann.
- **Weitere Abläufe:** Nach einer Wiederherstellung nennt das Pause-Banner Links zum Prüfen
  (Rechnungen, Kosten, Nummernkreise und vorgemerkte Entwürfe, Abo-Rechnungen, Kosten-Abos);
  neue Einmal-Codes lassen sich auch beim Erneuern kopieren oder als Textdatei speichern;
  Seitennamen „Erinnerungen einstellen“ und „Kosten-Abos“; „Buchen“ eines Kosten-Abos fragt wie
  jede Kosten-Erfassung „Kosten festschreiben?“; nach dem Speichern von „Meine Firmendaten“ geht
  es zurück zur Seite, von der du gekommen bist (nur Adressen innerhalb der App); Hilfe-Knopf und
  F1 führen zum selben Handbuch-Kapitel; der Abgang einer Anschaffung fragt mit Datum, Art,
  Erlös, Restbuchwert und Gewinn oder Verlust nach („Abgang buchen“), und die Vorschau zeigt
  ersetzte und nachgeholte AfA mit Vorzeichen.
- **Kosten, Anschaffungen, EÜR:** „Kosten festschreiben“ (statt „speichern“) mit Rückfrage, Beleg
  zuerst im Formular, „Korrigiert neu erfassen“ nach einem Storno übernimmt die Werte; Berater- und
  Mandantennummer im DATEV- und Steuerberater-Export; Hinweis zur Steuerkategorie K beim
  E-Rechnungs-Import; EÜR mit Hinweis zur Regelbesteuerung und zu Abweichungen vom Jahresabschluss;
  Abgang mit verbindlicher Restbuchwert-Vorschau; Steuer-Anlage zu §13b-Kosten mit Vorbelegung;
  AfA je Jahr vor dem Buchen, Abschlussprotokoll der Geschäftsjahre, Detailseite für
  Privatbewegungen; Alter der letzten externen Sicherung auf der Übersicht.
- **Einstellungen, Pakete, Kontakte:** Paket-Kategorien umbenennen und sortieren (ohne Ziehen),
  „Wiederverwenden“ einer früheren Paket-Version; Backup-Ergebnis je Ziel bleibt sichtbar,
  „ausgesetzt“ nach einer Wiederherstellung statt „fehlgeschlagen“; Rückfrage vor dem
  Wiederherstellen nennt alle Folgeschritte; Telefon und Steuernummer als „Pflicht für die
  E-Rechnung“ gekennzeichnet; umkehrbare Aktionen (archivieren, deaktivieren, pausieren) ohne
  Rückfrage, dafür mit „Rückgängig“.
- **Bedienung allgemein:** Bestätigungsdialog nennt die Aktion; Abbrechen ist kein Fehler mehr;
  Knöpfe behalten während einer Aktion den Fokus; Suchergebnisse und Ladezustände werden für
  Screenreader angesagt; Meldungen des Servers in der Sprache der Oberfläche (Daten als TT.MM.JJJJ,
  „Firmendaten“, ohne technische Regel-Kennungen).
- **Behoben:** Bestätigungsdialoge warfen beim Schließen einen Skriptfehler; eine abgebrochene
  Seitenübergangs-Animation erschien als unbehandelter Fehler; auf `/expenses/new` und in Tabellen
  mit versteckten Spaltenköpfen lief die Seite im Querformat seitlich über; die Statusliste der
  Backup-Seite stand in einer Zeile (Klassenkonflikt mit den Status-Pillen). Ein Backup-Ziel
  übernahm die „letzte erfolgreiche Kopie“ eines anderen oder entfernten Ziels mit derselben
  Bezeichnung (jetzt nur Kopien am selben Ort); die Meldung bei einer Zeitüberschreitung lud
  zum Wiederholen ein, obwohl die Aktion im Hintergrund weiterläuft — jetzt: erst in der Liste
  prüfen. Die Rücksprung-Adresse auf „Meine Firmendaten“ ließ sich mit Punkt-Segmenten auf eine
  fremde Seite lenken.

### Hinweise für Betreiber (Update von 2026.10)

- **`stop_grace_period` anheben:** Im eigenen Compose auf mindestens `90s`
  (bisheriges Beispiel 60 s). Das Herunterfahren dauert bis zu 75 s (offene
  Verbindungen 30 s, Tick und Jobs samt einem ausstehenden Lock-Backup 40 s, Datenbank
  5 s); bei 60 s bricht Docker ein eben ausstehendes Lock-Backup per SIGKILL ab.
- **SFTP-Ziele selbst rotieren:** Klein.Buch löscht auf SFTP-Zielen nichts; alte
  Backups dort entfernt der Betreiber (Platzbedarf im Handbuch „Speicherbedarf“).
- **fail2ban-Filter ersetzen:** Der Filter im Handbuch („Fehlersuche im Container“)
  ist jetzt an Zeilenanfang und -ende verankert; bestehende Filter bitte durch die neue
  Fassung ersetzen.
- **Zugriffsprotokoll:** Jede Zeile endet mit `ip=… req=… sess=…` (Client-IP,
  Aufruf-Kennung wie die `ref` in „Interner Fehler (Ref …)“, gekürzte
  Sitzungskennung). Eigene Auswertungen und Filter entsprechend anpassen.
- **Klartext-Exporte** (DATEV, ELSTER, Einzelaufstellung u. a.) liegen höchstens
  10 Minuten auf dem Server und verschwinden mit dem ersten Abruf; ein zweiter
  Download braucht einen neuen Export.
- **Backup-Format 3:** Neue Backups lesen nur 2026.11 und neuer; ein Rückfall auf
  2026.10 kann sie nicht einspielen. Das Pflicht-Backup vor der Migration
  entsteht im Format der Vorversion und bleibt mit 2026.10 einspielbar
  (Handbuch „Updates und Rückfall“).
- **Vorkomprimierte Oberfläche:** Die Dateien der Oberfläche kommen jetzt vorkomprimiert
  (Brotli/gzip, passend zum `Accept-Encoding` des Browsers) mit `Vary: Accept-Encoding`.
  Der Reverse-Proxy muss `Content-Encoding` und `Vary` unverändert durchreichen und darf
  diese Antworten nicht noch einmal komprimieren (Caddy `encode`, nginx `gzip on` und
  Traefik `compress` überspringen bereits kodierte Antworten von selbst). Für `/api/` und
  `/files/` keine Kompression am Proxy einschalten: Der Server liefert sie absichtlich
  unkomprimiert (Schutz gegen BREACH, Belege und Archive bitgenau).
- **Startbildschirm-App:** Ein Web-App-Manifest erlaubt „Zum Startbildschirm“ (ohne
  Service Worker, ohne Offline-Funktion; die CSP bleibt unverändert). Auf iOS hat die
  Startbildschirm-App eigene Cookies und braucht eine eigene Anmeldung.
- **Meldungstexte der API:** Die Texte im Feld `message` sind sprachlich überarbeitet
  (Daten als TT.MM.JJJJ, „Firmendaten“, ohne XRechnung-Regelkennungen; Wörter wie
  „verworfen“/„fertiggestellt“ auch in Anonymisierungs-Meldungen und im DSGVO-Auskunfts-PDF); die
  Codes (`code`) sind unverändert. Wer eigene Auswertungen auf Meldungstexte stützt, bitte anpassen.
- **Eigene PDF-Vorlagen (Typst 0.15):** `json.decode(…)` gibt es nicht mehr;
  Daten lesen mit `json(bytes(sys.inputs.at("data-json")))`. Eine alte Vorlage
  meldet Klein.Buch beim Rendern mit genau diesem Ersatz.
- **Abgeschlossene Jahre mit aktivierten Kosten:** Der Snapshot-Abgleich meldet
  für Jahre, in denen eine Kostenposition zu einer Anlage wurde, eine
  Abweichung — das ist die frühere Doppelzählung (Kosten und AfA). Der
  festgeschriebene Stand bleibt; ob die abgegebene Erklärung zu berichtigen ist,
  klärt der Steuerpflichtige mit dem Steuerberater.
- **Arbeitsspeicher (`mem_limit`):** Die Faustregel rechnet jetzt den Java-Sidecar
  mit: 512 MiB + 1 GiB (Java, zwei Läufe) + 2 × größtes Backup. `mem_limit: 2g`
  reicht damit für Backups bis etwa 250 MiB, `3g` bis etwa 750 MiB (Handbuch
  „Speicherbedarf“). Lock-Backups laufen im Hintergrund, während schon die nächste
  Rechnung festgeschrieben wird.
- **Anlagen aus Kosten bei §19:** Anschaffungskosten sind als Kleinunternehmer der
  Bruttobetrag (§ 9b Abs. 1 EStG). Vor 2026.11 aus Kosten übernommene Anlagen tragen
  den Nettobetrag; solange für sie keine AfA gebucht ist, lassen sie sich korrigieren,
  sonst klärt der Steuerpflichtige die Korrektur mit dem Steuerberater.
- **Anlagen aus Kosten mit abweichendem Betrag:** Ältere Anlagen, deren
  Anschaffungskosten nicht dem Betrag ihrer Kosten entsprechen (Teil-Aktivierung,
  Nebenkosten), ändert Klein.Buch nicht; die EÜR-Ausfüllhilfe nennt die betroffenen
  Kosten. Die Korrektur klärt der Steuerpflichtige mit dem Steuerberater. Bei
  Regelbesteuerung zählt die Vorsteuer aktivierter Kosten jetzt in der EÜR mit;
  abgeschlossene Jahre zeigen dann eine Abweichung im Snapshot-Abgleich.
- **Steuerstatus der Kosten nach dem Leistungsbezug:** Vorsteuer und Anschaffungskosten
  aus Kosten richten sich jetzt nach dem §19-Status im Jahr des Belegdatums, nicht des
  Zahlungsjahres. Wer auf §19 verzichtet hat, sieht für Kosten, deren Beleg und Zahlung
  in Jahren mit verschiedenem Status liegen, eine andere Aufteilung (brutto/netto,
  Kz 185, DATEV-BU-Schlüssel) und einen Hinweis in der Ausfüllhilfe; abgeschlossene
  Jahre mit solchen aktivierten Kosten zeigen eine Abweichung im Snapshot-Abgleich.

### Changed

- **Abo-Läufe im Dauerbetrieb (Lauf F, ADR 0040):** Verpasste Termine holt
  Klein.Buch nur noch bis drei je Abo bzw. Vorlage selbst nach; bei Abo-Rechnungen
  folgt nur der jüngste Termin der Automatik (ausstellen, versenden), ältere
  entstehen als Entwurf und gehen nie automatisch hinaus. Bei mehr offenen
  Terminen fragt Klein.Buch über einen Hinweis und „Rückstand …“ in der Liste
  (nachholen bis 24 oder ab heute fortsetzen); dieselbe Wahl beim Fortsetzen
  einer pausierten Reihe. Neue Abo-Rechnungs-Vorlagen beginnen nicht in der
  Vergangenheit.
- **Pause bei falscher Uhr:** Steht die Server-Uhr vor dem letzten Lauf oder
  mehr als 400 Tage danach, hält Klein.Buch Abo-Läufe und Auto-AfA an, bis der
  Inhaber im Banner freigibt (Re-Auth).
- **Kosten-Abos im Modus „Erinnerung“** erzeugen am Stichtag den Hinweis
  „Abo fällig“; das Handbuch beschreibt die Zahlungslogik richtig (Position
  „noch nicht bezahlt“).
- **Keine Schreibwirkung beim Öffnen von Seiten:** Die Seite „Wiederkehrende
  Rechnungen“ löst beim Öffnen keinen Lauf mehr aus; Standard-Zahlungskonten legt
  der Server einmal bei der Einrichtung bzw. beim ersten Entsperren an.
- **Anonymisieren eines Kontakts** pausiert seine Abo-Rechnungs-Vorlagen und
  Kosten-Abos.
- **Uhr injizierbar:** Scheduler und Rechnungsweg lesen die Zeit über die Uhr
  des Anwendungskontexts (Tests mit fester Uhr über alle Threads).
- **Pause nach einer Wiederherstellung (Lauf F, ADR 0040):** Nach dem Einspielen
  eines Backups bleiben Abo-Rechnungen, Abo-Kosten, Auto-AfA und Versand
  angehalten, und Backups gehen nur in den lokalen Ordner, bis der Inhaber im
  Banner bestätigt, dass dies die produktive Instanz ist (Re-Auth). Eine
  Probe-Instanz verschickt damit nichts und schreibt keine älteren Stände auf
  die externen Ziele.
- **Tages-Backup kalenderfest:** genau eines je Kalendertag (Europe/Berlin) ab
  einer einstellbaren Uhrzeit (Standard 02:30, Einstellungen → Backups), nach
  einer Sperre genau eines; der Backoff nach einem Fehlschlag übersteht
  Neustarts. Die Einrichtung legt nur noch ein Erst-Backup an.
- **Aufbewahrung nach Kalendertagen:** Je Tag, Monat und Jahr bleibt das
  jüngste Backup (Floor 7/3/1, Ordner-Ziele 30/12/7); viele Festschreibungen an
  einem Tag verdrängen die Vortage nicht mehr, die Monatsklasse hängt nicht mehr
  am Monatsersten. Auf SFTP-Zielen löscht Klein.Buch nichts (Rotation beim
  Betreiber, Platzbedarf im Handbuch). Während einer Pause ruht die Rotation.
- **Nach dem Entsperren gestaffelt:** Der erste Lauf folgt frühestens eine
  Minute nach der Anmeldung, ohne Archiv-Vollscan; Abo-Läufe eines Durchgangs
  ergeben zusammen genau ein Backup. Das Container-Log meldet den gesperrten
  Zustand beim Serverstart und alle 6 h; nach mehr als zwei Tagen Sperre gibt es einen
  Hinweis.
- **Herunterfahren:** Der Scheduler hält beim Signal sofort an; Tick und Jobs
  haben höchstens 30 s, dazu ein ausstehendes Lock-Backup; das Herunterfahren
  dauert höchstens 75 s.
  `stop_grace_period` im Compose-Beispiel und in der Doku auf 90 s.
- **Aufräumen im Betrieb:** Reste in `/data/tmp` und abgebrochene
  Backup-Teildateien entfernt Klein.Buch stündlich (älter als 6 h), nicht nur
  beim Serverstart.
- **Zeitangaben in Berliner Zeit:** Protokolle (Audit, E-Mail, Backup),
  Hinweise, Geschäftsjahr, Versionsprüfung, Versandprotokolle von Angeboten und
  Rechnungen und die Backup-Liste zeigen Datum und Uhrzeit in Europe/Berlin,
  unabhängig von der Zeitzone des Geräts; die Backup-Liste zeigt die Zeitpunkte
  nicht mehr roh. Die Spalte „Zeitpunkt (UTC)“ im Audit-Protokoll heißt
  „Zeitpunkt“.
- **Neue Fassungen geänderter Vorlagen:** Eine `.neu`-Datei in `/inputs` legt
  Klein.Buch je Image-Fassung nur einmal an und meldet sie einmal; nach dem
  Vergleich darf sie gelöscht werden.
- **Meldungen für den Server-Betrieb:** „Bitte den Container neu starten“,
  „das Image der passenden Version einsetzen“ und „die Instanz entsperren“
  statt Hinweisen auf App-Neustart und App-Update. Doku, ADRs (Amendments zu
  0003, 0023, 0027, 0033, 0039) und Handbuch beschreiben Serverstart und
  Entsperren statt des Programmstarts der Desktop-Fassung.
- **Nur noch die „Hinweise“:** Der ungenutzte Weiterleitungskanal für
  OS-Benachrichtigungen aus der Desktop-Fassung ist entfernt.
- **Migration 0032 (Schema 32, Lauf F, ADR 0039 Nr. 124):** Nummerierte
  Rechnungs- und Storno-Entwürfe lassen sich verwerfen (`invoices_void_draft`,
  Re-Auth, Grund Pflicht) — ohne Löschen: Zeitpunkt und Grund stehen am Entwurf
  und im Audit, die Nummer bleibt verbraucht, ein verworfener Entwurf ist
  endgültig und blockiert die DSGVO-Anonymisierung nicht mehr; zurückgezogene
  Angebots-Entwürfe ebenso. Die Datenbank erzwingt zusätzlich: höchstens ein
  festgeschriebener Storno je Rechnung, Storno endgültig, Positionen
  festgeschriebener Belege unveränderlich, Archiv-Einträge nie gelöscht,
  Anlagen-Abgang endgültig, höchstens ein Standard-Zahlungskonto (Bestand mit
  Audit bereinigt). Die Integritäts-Historie speichert „fehlt“ und „nicht
  lesbar“; die Regel „Kosten-Abo fällig“ ist abschaltbar.
- **Prüfung beim Entsperren:** Verletzte Fremdschlüssel oder eine gescheiterte
  Vorab-Prüfung einer Migration blockieren das Entsperren mit klarer Meldung
  (409 `db_integrity_failed`, Einzelheiten im Log). Nach einem Update prüft
  Klein.Buch das schon vor dem Pflicht-Backup und vor der Umstellung; die
  Datenbank bleibt dann auf dem alten Schema, und das vorige Image öffnet sie
  weiter (Lauf F2c, ADR 0039 Nr. 126). Schlägt die Prüfung erst nach einer
  Umstellung an, sagt die Meldung das und das Handbuch nennt den Rückweg.
- **Verworfene Entwürfe und DSGVO (Lauf F2c):** Die Anonymisierung eines
  Kontakts entfernt auch den Empfänger aus seinen verworfenen
  Rechnungs-Entwürfen und nie ausgestellten, zurückgezogenen Angeboten sowie
  Bezeichnung und Notiz seiner Abo-Vorlagen; Nummer, Betrag und Grund bleiben.
  Die Auskunft enthält Abo-Vorlagen, Kosten-Abos und Anlagen des Kontakts und
  zeigt verworfene Entwürfe als „verworfen“. Handbuch und `gobd.md` sagen
  ehrlich, was nach Ablauf der Frist bleibt.
- **Angebote (Lauf F2c):** Wird der aus einem Angebot entstandene Entwurf
  verworfen, ist das Angebot wieder „angenommen“ und lässt sich neu umwandeln.
  Zurückziehen braucht einen Grund (mindestens 10 Zeichen), läuft mit Audit in
  einer Transaktion und nie parallel zur Umwandlung; ein umgewandeltes Angebot
  bleibt umgewandelt.
- **Anlagen (Lauf F2c):** Eine veräußerte Anlage lässt sich weder bearbeiten
  noch ihre AfA zurücksetzen; beim Abgang holt Klein.Buch fehlende Vorjahres-AfA
  nach, und der Jahresabschluss schreibt auch veräußerte Anlagen fest.
- **Exporte abgeschlossener Jahre (Lauf F2c):** Bei einer Abweichung vom
  Abschluss tragen das EÜR-PDF (Kasten), die ELSTER-Ausfüllhilfe
  (Kommentarzeile) und die Einzelaufstellung (Begleitdatei) den Vermerk in der
  Datei. Das Steuerberater-Paket listet nicht festgeschriebene Rechnungs- und
  Storno-Nummern (`nicht-festgeschriebene-nummern.csv`).
- **Audit-Log (Lauf F, ADR 0039 Nr. 125):** Details sind immer gültiges JSON,
  auch mit Anführungs- oder Steuerzeichen in Namen und Gründen. Der Akteur
  unterscheidet `owner` (über die Sitzung), `scheduler` (automatische Läufe) und
  `system`. Jeder EÜR-Export (PDF, DATEV, ELSTER, Einzelaufstellung,
  Steuerberater-Paket) steht mit Jahr, Dateiname und Zählwerten im Protokoll,
  ohne Inhalte.
- **Abgeschlossene Jahre:** Anzeige und Exporte vergleichen die EÜR mit dem beim
  Abschluss festgeschriebenen Stand. Bei einer Abweichung gibt es einen
  dringenden Hinweis, einen Audit-Eintrag je Export und im Steuerberater-Paket
  eine Gegenüberstellung in der `LIESMICH.txt`; blockiert wird nichts.
- **Geschäftsjahres-Abschluss:** wartet auf laufende Buchungen, und Buchungen
  während des Abschlusses warten und werden danach erneut geprüft; Festschreiben,
  Snapshot, Protokoll und Audit laufen in einer Transaktion. Das Audit nennt die
  Zahl der Rechnungen, Stornos, Kosten, Privatbewegungen und offenen Entwürfe;
  die Übersicht meldet offene Entwürfe vorab.
- **DSGVO-Auskunft:** findet Mails auch an frühere Adressen der Person (aus
  ihren Belegen) und unabhängig von Groß-/Kleinschreibung; Katalog-Mails stehen
  mit Kontaktbezug im Versandprotokoll; Audit-Bezüge nur mit passendem Typ; eine
  feste Zahl von Datenbank-Abfragen statt einer je Beleg.
- **Archiv:** Jede neue Archivdatei wird vor dem Sichtbarwerden zurückgelesen
  und gegen ihren Prüfwert verglichen; auf Dateisystemen ohne Hardlinks legt
  Klein.Buch sie mit `RENAME_NOREPLACE` ab (nie überschreiben).

- **Storno im Folgejahr:** Ein Storno-Beleg bekommt Geschäftsjahr und Nummer aus
  seinem eigenen Datum (`ST-2027-0001` vom Januar 2027 zu einer Rechnung vom
  Dezember 2026) statt aus der Original-Rechnung (GoBD Rz. 51). Ein
  abgeschlossenes Vorjahr bekommt keine neuen Belege mehr. Bereits erzeugte
  Stornos bleiben unverändert.
- **Storno-PDF:** Überschrift „Stornorechnung“, Zeile „Storno zur Rechnung
  <Nummer> vom <Datum>“, Betrag als „Stornobetrag“ und kein
  Überweisungshinweis mehr — in allen vier Rechnungsvorlagen. Eigene Vorlagen
  können das neue Feld `data.invoice.storno_of` nutzen und rendern ohne es
  unverändert.
- **Backup nach dem Festschreiben im Hintergrund (Lauf F3a, ADR 0040 Nr. 50):**
  Festschreiben, Stornieren, Kosten, Anlagen, AfA und Jahresabschluss warten
  nicht mehr auf das Backup; es entsteht wenige Sekunden später im Scheduler
  (mehrere Festschreibungen kurz hintereinander ergeben ein Backup). Ein
  Vermerk in der Datenbank sorgt dafür, dass es nach Sperren oder Neustart
  nachgeholt wird.
- **Backup-Format 3 (Lauf F3a, ADR 0039 Nr. 127):** Der lesbare Kopf der
  `.kbk` (Versionen, Zeitpunkt, Prüfsumme) ist an die Verschlüsselung gebunden;
  ein veränderter Kopf fällt beim Einspielen auf. Backups im Format 1 und 2
  bleiben einspielbar. Erstellen und Einspielen brauchen deutlich weniger
  Arbeitsspeicher (Entschlüsseln etwa einmal statt dreimal die Backup-Größe,
  das Sicherheits-Backup danach; Faustregel im Compose-Beispiel jetzt
  512 MiB + 2 × größtes Backup).
- **Backups herunterladen (Lauf F3a):** Im Backup-Verlauf lädt „Herunterladen“
  (Re-Auth) die verschlüsselte `.kbk` aus dem lokalen Ordner oder einem
  Ordner-Ziel — für Umzug und Notfall ohne externes Ziel.
- **Off-Site-Status „veraltet“ (Lauf F3a):** Hat ein externes Ziel noch keine
  Kopie oder ist seine letzte älter als sieben Tage, zeigt die Übersicht eine
  Aufgabe; Healthcheck und Einstellungen melden `stale`.
- **Anmeldung und Sitzungen (Lauf F4a, ADR 0039 Nr. 130):** Einstellungen →
  Sicherheit zeigt die angemeldeten Sitzungen und meldet auf Wunsch alle anderen
  ab (Re-Auth). Nach 100 Fehlversuchen in Folge ist die Anmeldung mit App-Code
  von unbekannten Geräten gesperrt, bis eine Anmeldung mit Einmal-Code oder von
  einem bekannten Gerät gelingt (Eintrag unter „Hinweise“). Neue Daten-Passwörter prüft der
  Server gegen eine eingebettete Sperrliste, einfache Muster und Firmenname bzw.
  E-Mail (höchstens 1024 Zeichen); bestehende bleiben gültig. Eine Anmeldung, die
  wegen einer Datenbank-Umstellung länger dauert, antwortet nach 25 s mit 202
  und wird abgeholt statt abzubrechen. Firmendaten verlangen eine Bestätigung
  nur noch bei Änderung von IBAN, BIC oder §19-Status (nicht in den ersten 5 min
  nach der Anmeldung); DATEV-, ELSTER- und PDF-Export sowie das Aktivieren bzw.
  Deaktivieren von Zahlungskonten verlangen sie neu. „Datenbank sperren“ leert
  zusätzlich gespeicherte Antworten, Job-Ergebnisse und offene Anmeldungen;
  Passphrase und Zugangsdaten liegen im Speicher als überschreibbare Werte;
  Idempotenz-Schlüssel gelten je Anmeldung; verbrauchte TOTP-Codes stehen als
  `reason=totp_replay` im Log.
- **HTTP-Server und Eingaben (Lauf F4b, ADR 0039 Nr. 131):** Höchstens 256
  offene Verbindungen, Gegenstellen, die 30 s lang nichts mehr annehmen, werden
  getrennt; `/legal/` wird gestreamt. Eine Panik in einem Handler ergibt 500 mit
  Referenz statt eines Verbindungsabbruchs. Nicht leere Anfragekörper an die API
  müssen `application/json` sein (sonst 415). `/healthz` zeigt Zustand und
  Off-Site-Status nur direkt über Loopback, nicht über einen Proxy auf demselben
  Host. Textantworten tragen `charset=utf-8`. Klartext-Exporte liegen höchstens
  10 min statt 1 h auf dem Server und verschwinden mit dem ersten Abruf. Die Art
  jedes Commands (Lesen/Ändern/Job) steht ausdrücklich in der Registry statt aus
  dem Namen abgeleitet. Beim Start prüft der Server den Java-Sidecar (Log und
  Eintrag unter „Hinweise“ beim Entsperren, kein Abbruch).
- **Logs und Fehlermeldungen (Lauf F4c, ADR 0039 Nr. 132):** Das
  Zugriffsprotokoll nennt Client-IP (`ip=`), Aufruf-Kennung (`req=`, gleich der
  `ref` in „Interner Fehler (Ref …)“) und eine gekürzte Sitzungskennung
  (`sess=`); erfolgreiche Re-Auth steht mit IP im Log. Jedes Log-Ereignis ist
  genau eine Zeile (Zeilenumbrüche in Werten escaped). SMTP- und SFTP-Hosts
  müssen ein Hostname oder eine IP-Adresse sein. Ein gescheiterter Auto-Versand
  nennt auf WARN keine Empfängeradresse mehr (Volltext auf DEBUG, im Hinweis und
  im E-Mail-Protokoll). Fehlermeldungen an die Oberfläche nennen keine
  absoluten Serverpfade mehr. Der fail2ban-Filter im Handbuch ist verankert —
  bestehende Filter bitte ersetzen. Mustang bleibt auf 2.23.0 (auch 2.26.0
  bündelt Jackson 2.22.2); die befristeten Scan-Ausnahmen bleiben bis 30.04.2027.
- **Nachbesserung Anmeldung und Server (Lauf F4d, ADR 0039 Nr. 133):** Der
  Hinweis zur Sperre der App-Code-Anmeldung unterstellt nicht mehr, dass das
  Daten-Passwort bekannt ist (die Sperre ist schon mit falschen App-Codes
  auslösbar). Neu: Drei Anmeldungen in Folge mit gültigem App-Code, aber
  falschem Daten-Passwort ergeben einen dringenden Hinweis mit der Empfehlung,
  den App-Code neu einzurichten. Höchstens 32 offene Verbindungen je
  Gegenstelle (vertraute Proxys ausgenommen). Passphrasen und Passwörter in
  Command-Argumenten erscheinen in Debug-Ausgaben nur als `***`. Doku: Der
  Download-Link eines Backups gilt höchstens 10 Minuten.

- **Beiträge Dritter:** `CONTRIBUTING.md` formuliert „vorerst keine
  Code-Beiträge“ als Entscheidung (Fehlerberichte, Wünsche, Sicherheitsmeldungen
  willkommen; ADR 0014, Amendment 2026-10-24).
- **Quellangebot:** Das Release mit dem Quelltext der Komponenten Dritter wird vor
  dem Image veröffentlicht (nicht mehr als Entwurf); das Image nennt die
  Adresse seines Quellarchivs im Label `de.klein-buch.source-archive`; README
  und Handbuch weisen darauf hin; Release-Assets werden nie ersetzt.
- **Abhängigkeiten:** Rust-Bibliotheken auf aktuelle Minor-/Patch-Stände (u. a.
  hyper 1.12, uuid 1.29, serde_json 1.0.154, lettre 0.11.23, chrono 0.4.45),
  Svelte 5.57 und svelte-check 4.7, GitHub Actions auf die Fassungen mit
  Node 24 (checkout 7, setup-node 7, upload-artifact 7, build-push 7, setup-buildx
  4, pnpm/action-setup 6). Basis-Images bleiben auf den LTS-Linien Node 22 und
  Java 21; Dependabot schlägt dort keine Major-Sprünge mehr vor.
- **Rechnungs-Entwürfe bearbeiten (Lauf F5a, ADR 0039 Nr. 134):** „Bearbeiten“
  auf der Detailseite öffnet einen offenen Entwurf im bekannten Formular; die
  Nummer bleibt, das Datum bleibt im Geschäftsjahr der Nummer. Nicht für
  ausgestellte, verworfene, Storno- und laufende Abo-Entwürfe. Audit
  `invoice.draft_updated`. Geänderte Paket-Positionen verlieren ihren Paket-Bezug
  (der Server prüft das jetzt auch beim Anlegen).
- **§19-Status beim Festschreiben eingefroren:** Rechnung und Angebot halten den
  Status fest, unter dem sie geprüft und gerendert wurden; das Angebots-PDF und
  sein Klausel-Check nutzen ihn. Ein Storno übernimmt den Status der stornierten
  Rechnung, auch nach einem Wechsel.
- **Umsatzsteuer je Steuersatz aus der Nettosumme** (EN 16931 BR-CO-17) statt als
  Summe gerundeter Positionssteuern — bei Regelbesteuerung mit vielen
  Kleinpositionen stimmen Beleg und E-Rechnung jetzt auf den Cent; bei §19
  ändert sich nichts.
- **Regelbesteuerung:** Steuerart E gibt es nur als Kleinunternehmer; Formulare
  und Paket-Positionen setzen E auf S mit 19 %, der Satz ist an jeder Position
  (auch aus Paketen) einstellbar.
- **Eigene Rechnungsvorlage ohne Storno-Darstellung:** Stornos entstehen dann
  mit der eingebauten Vorlage, ein Eintrag unter „Hinweise“ erklärt es.
- **Fehlermeldungen der Datenbank-Schutzregeln** erscheinen deutsch und
  verständlich (mit Referenz), der technische Text steht nur im Log.
- **Übersicht über Mitternacht:** Datum, Jahr und Fälligkeiten springen in einem
  offenen Tab um Mitternacht (Europe/Berlin) auf den neuen Tag.
- **ELSTER-Ausfüllhilfe nach dem Vordruck des Jahres (Lauf F5b, ADR 0039
  Nr. 135):** Zeile und Kennzahl (Kz) der amtlichen Anlage EÜR 2024/2025 bzw.
  2026 (dort „Waren“ in Zeile 29 und alle weiteren Ausgaben-Zeilen eins tiefer);
  CSV mit Spalte `Kennzahl`, PDF und Export-Seite mit Spalte „Kz“. Kategorien mit
  eigener Zeile (Telefon/Internet, Fortbildung, Versicherungen, Software,
  Bürobedarf, Werbung, Kfz) stehen dort statt in „übrige Betriebsausgaben“.
- **Regelbesteuerung in der EÜR:** Einnahmen netto in Zeile 15, Umsatzsteuer in
  Zeile 17, steuerfreie in Zeile 16, Ausgaben netto mit Vorsteuer — nach dem
  Status, mit dem die Rechnung ausgestellt wurde. Was fehlt (Voranmeldung,
  Zahlungen an das Finanzamt, USt auf Anlagenverkäufe), nennen Ausfüllhilfe,
  PDF, LIESMICH und der Verzicht-Dialog.
- **DATEV-Stapel:** Geldkonto je Zahlungskonto (Bank, Kasse, Geldtransit für
  PayPal/Stripe); beim Erfassen einer Zahlung wählst Du das Konto. Erlöse bei
  Regelbesteuerung je Steuersatz auf die Automatikkonten, Kosten mit
  BU-Schlüssel; Sachkontenrahmen im Kopf, Berater-/Mandantennummer optional.
- **Versand an einen anderen Empfänger (Lauf F5c, ADR 0039 Nr. 136, W14-009):**
  Rechnungs- und Angebotsversand an eine andere als die hinterlegte Adresse
  verlangt eine Bestätigung mit Passphrase und App-Code (eine Anmeldung der
  letzten 5 min genügt); Betreff und Text bleiben frei.
- **ZUGFeRD-Profil XRECHNUNG (R3-017):** Das Rechnungs-PDF trägt das Profil, das
  zur eingebetteten XRechnung 3.0 passt (XMP `XRECHNUNG`, Anhang
  `xrechnung.xml`); bisher stand „EN 16931“ im Container. Mit KoSIT und Mustang
  geprüft.
- **Gespeicherte SMTP-Ports** folgen derselben Allowlist wie die
  Verbindungstests (`KLEIN_BUCH_ALLOWED_TEST_PORTS`); private und
  Loopback-Adressen bleiben erlaubt.
- **Druck-PDF mit AGB:** Aktive Inhalte hochgeladener Rechtsdokumente
  (JavaScript, Programmstart, Formularversand, eingebettete Dateien) werden beim
  Zusammenführen entfernt; der Catalog ist der des Angebots.
- **PDF-Erzeugung mit Typst 0.15 (Lauf F6b, ADR 0039 Nr. 139):** Rechnungsbild aller
  Vorlagen gegen 0.13 per Text- und Pixelvergleich geprüft (gleicher Text, nur
  unsichtbare Kantenabweichungen in „Minimal“). PDFs sind jetzt getaggt
  (barrierearm) und tragen ein Erstellungsdatum. Eigene Vorlagen lesen die Daten mit
  `json(bytes(sys.inputs.at("data-json")))`; das entfernte `json.decode` meldet
  Klein.Buch mit dem Ersatz.
- **Krypto-Crates:** sha2 0.11, hmac 0.13, sha1 0.11 (Hashes, TOTP und Gerätecookie
  bit-genau wie bisher, Known-Answer-Tests); `pageant` 0.2.4 und `cmov` 0.5.4
  (Dependabot-Meldungen). tera bleibt bei 1 (Mail-Vorlagen der Betreiber).
- **Frontend-Werkzeugkette (Lauf F6c, ADR 0039 Nr. 140):** SvelteKit 3.1, Vite 8.3
  (Rolldown), Vitest 5, TypeScript 6, vite-plugin-svelte 7, adapter-static 4.
  Konfiguration in `vite.config.js` statt `svelte.config.js`; Importe über `#lib/…`
  statt `$lib`, Seitenzustand über `$app/state` statt `$app/stores`. Die Lizenzliste
  unter `/legal/` entsteht weiter aus dem ausgelieferten Bundle (jetzt mit `devalue`,
  ohne `esm-env`). Nach einem Image-Update lädt die Oberfläche bei einem Ladefehler
  neu, statt veraltete Dateien anzufordern. Alle gemeldeten Fassungen (Dependabot)
  sind aus dem `pnpm-lock.yaml` verschwunden; `pnpm audit` meldet nichts.
- **Node 24 LTS** für den SPA-Build im Image und in der CI (`engines.node >=24.11.0`);
  die CI baut die SPA jetzt auch im Job `frontend` und prüft die Lizenzliste.
- **CI und Betrieb (Lauf F7a, ADR 0039 Nr. 142):** SMTP-E2E-Test gegen Mailpit
  (`axllent/mailpit`, per Digest) statt MailHog; `check_base_images.py` verlangt einen
  Digest auch für Service-Images der Workflows und feste Images der CI-Skripte. Der
  Release-Trockenlauf prüft das vom Image erzeugte Rechnungs-PDF mit veraPDF auf
  PDF/A-3b (R3-018). `pnpm audit` blockiert ab „high“ auch bei Entwicklungsabhängigkeiten.
  Tests nutzen nur Dokumentations-Adressen und neutrale Namen; `test_addresses_test`
  prüft das positiv. Der Wächtertest bildet den Veröffentlichungs-Filter vollständig nach
  (Namen und Dateimuster). Die Prüfliste der Lizenzseite im Image-Smoke wird gegen das
  Bundle abgeglichen. Betriebshandbuch: Abschnitt „Monitoring und Alarmierung“ (ASVS
  8.1.4, V11.1.7/8 entschieden); Sidecar-Grenzen in `modules.md` dokumentiert.
- **§13b bei Kosten (Lauf F7b, ADR 0039 Nr. 143):** Kosten mit dem Schalter
  „§13b-Reverse-Charge“ müssen ohne Umsatzsteuer erfasst werden (Netto = Brutto);
  mit USt-Betrag lehnt Klein.Buch sie mit Erklärung ab. Eine E-Rechnung, die
  §13b-Positionen neben ausgewiesener Umsatzsteuer enthält, wird ohne den
  Schalter und mit einem Hinweis in der Notiz vorbelegt. Das Handbuch („Kosten
  erfassen“, „Kleinunternehmer-Regelung“, Glossar, FAQ, Disclaimer) erklärt jetzt
  richtig, dass §13b auch Kleinunternehmer trifft (Voranmeldung, Zahlung, kein
  Vorsteuerabzug) und dass Klein.Buch die Steuer nicht berechnet.
  Im Formular stellt der Schalter den USt-Satz auf 0 % und sperrt die übrigen Sätze;
  der Hinweis daneben nennt USt-Betrag 0 statt freier Eingabe (F7-fix, Nr. 144).
- **Doku nach Lauf F (F7b):** Zähl- und Versionsangaben in README, ARCHITECTURE,
  PRD (Revision 9, Nachträge R9.4: Schema 32, Backup-Format 3, Lock-Backup im
  Hintergrund, Re-Auth, ELSTER-Kennzahlen, DATEV-Geldkonten, Storno-Überhang,
  Mustang-Profil XRECHNUNG, Werkzeugkette) und den CLAUDE.md-Dateien nachgezogen;
  `data-model.md` dokumentiert die drei `ON DELETE CASCADE`; ADR 0039 Nr. 57
  berichtigt (`encrypt_db_file_in_place` nur noch Test-Hilfe).

### Fixed

- **Anschaffungsjahr beim Korrigieren einer Anlage (Lauf F8):** Wird das Anschaffungsdatum
  einer noch nicht festgeschriebenen Anlage in ein anderes (offenes) Jahr korrigiert, folgt
  jetzt auch das Anschaffungsjahr (AfA-Beginn, Jahresfilter); die Anlagennummer bleibt.
- **Nachbesserung F8e (ADR 0039 Nr. 148 (5), Bekannte Grenze (b)):**
  - *Anschaffungsdatum und Belegdatum:* Liegen das Anschaffungsdatum einer Anlage aus
    Kosten (Lieferung, § 9a EStDV) und das Belegdatum der Kosten in Jahren mit
    unterschiedlichem §19-Status, lehnen Anlegen und Bearbeiten mit beiden Daten,
    beiden Status und Verweis an die Steuerberatung ab; bestehende Anlagen in dieser
    Lage gelten nach einem Wechsel als nicht korrigierbar.
  - *Wegfall von §19 mitten im Jahr (100.000-€-Grenze):* als bekannte Grenze
    dokumentiert. Hinweis und Ausfüllhilfe raten in diesem Fall nicht mehr zur
    Korrektur der Anlagen und nennen die zu hohe Vorsteuer in Kz 185; das Handbuch
    beschreibt das Vorgehen bei Einnahmen, Kosten und Anlagen und sagt, dass
    Klein.Buch die Umsatzgrenzen nicht überwacht.
- **Nachbesserung F8d (ADR 0039 Nr. 148):**
  - *Steuerstatus der Kosten nach dem Leistungsbezug:* Ob Kosten brutto oder netto mit
    Vorsteuer zählen (Ausfüllhilfe Kz 185, EÜR-Vorsteuer aktivierter Kosten,
    DATEV-BU-Schlüssel) und welche Anschaffungskosten eine Anlage aus Kosten hat
    (§ 9b Abs. 1 EStG), richtet sich nach dem §19-Status bei Leistungsbezug (Jahr des
    Belegdatums; § 15 Abs. 1 UStG, Abschn. 19.5 UStAE), nicht mehr nach dem
    Zahlungsjahr. Die Ausgabe bleibt im Zahlungsjahr (§ 11 EStG). Die Ausfüllhilfe
    nennt Kosten mit abweichendem Status.
  - *Zahldatum aktivierter Kosten:* Die F8c-Sperre entfällt; das Zahldatum ändert den
    Status nicht mehr. Das Belegdatum bleibt unveränderlich.
  - *§19-Wechsel nach dem Aktivieren:* Ein Verzicht oder eine Rückkehr wird nicht
    blockiert; ein Eintrag unter „Hinweise“ nennt jede betroffene Anlage mit richtigem Betrag und
    Handlungsweg (korrigieren bzw. Steuerberatung), die Ausfüllhilfe ebenso.
- **Nachbesserung F8c (ADR 0039 Nr. 147):**
  - *Zahldatum aktivierter Kosten:* Ein neues Zahldatum (oder das Entfernen), das den
    §19-Status der Kosten wechselt, lehnt der Server ab, wenn die Anschaffungskosten
    der Anlage dann nicht mehr passen — sonst setzten EÜR, Kz 185 und DATEV eine
    Vorsteuer an, die schon in der AfA steckt. Prüfung, Änderung und Protokoll laufen
    in einer Transaktion. Der Hinweis der Ausfüllhilfe zu abweichenden
    Anschaffungskosten ist neutral gefasst.
  - *§13b als Kleinunternehmer:* Die selbst geschuldete Steuer auf eine Anlage
    erfasst Du bei der Zahlung ans Finanzamt als eigene Kosten und übernimmst sie als
    eigene Anlage (gleiches Datum, gleiche Methode); die Rechnung selbst wird zum
    Rechnungsbetrag aktiviert. Einen höheren Betrag an der Rechnung lehnt der Server
    mit diesem Hinweis ab; das Formular zeigt ihn an.
- **Nachbesserung F8b (ADR 0039 Nr. 146, ADR 0040 Nr. 64):**
  - *Anlage aus Kosten nur mit genau dem Betrag der Kosten:* als Kleinunternehmer
    brutto, bei Regelbesteuerung netto (§ 9b Abs. 1 EStG). Teil-Aktivierungen und
    Mehrbeträge lehnt der Server ab — die EÜR nimmt die ganze Kostenposition heraus,
    ein Rest wurde sonst nie abgezogen und das Anlagekonto ging nie auf 0.
    Sammelrechnungen vorher in getrennte Kostenpositionen aufteilen; Nebenkosten aus
    einem eigenen Beleg als eigene Anlage übernehmen.
  - *Vorsteuer aktivierter Kosten (Regelbesteuerung):* zählt jetzt als
    Betriebsausgabe im Zahlungsjahr (EÜR, Ausfüllhilfe Kz 185); bisher fiel sie mit
    der Kostenposition heraus. Der DATEV-Stapel buchte sie schon richtig.
- **Schlussprüfung Lauf F (F8, ADR 0039 Nr. 145, ADR 0040 Nr. 62/63):**
  - *Anlage aus Kosten:* Kosten eines abgeschlossenen Jahres (Beleg- oder
    Zahlungsjahr) und stornierte Kosten lassen sich nicht mehr als Anlage
    übernehmen — das Aktivieren änderte sonst die EÜR des abgeschlossenen Jahres.
    Nummer, Anlage, Verweis und Protokoll entstehen in einem Schritt (keine
    doppelte Aktivierung, keine Nummer ohne Spur); die Quell-Kosten einer Anlage
    lassen sich nicht mehr tauschen.
  - *§ 9b EStG:* Als Kleinunternehmer übernimmt das Formular die Anschaffungskosten
    brutto (die nicht abziehbare Umsatzsteuer ging seit F5 sonst in keinem Jahr in
    die EÜR ein, und auf dem DATEV-Anlagekonto blieb sie stehen), bei
    Regelbesteuerung netto; Anschaffungsdatum = Belegdatum. Der Methodenvorschlag
    prüft die GWG-Grenze am Nettobetrag. Genau netto bei §19 (bzw. genau brutto bei
    Regelbesteuerung) lehnt der Server ab.
  - *Privatentnahme einer Anlage:* Abgangsart „Privatentnahme / Geschenk“ mit
    Pflichtfeld Entnahmewert (Teilwert; 0 bei einem betrieblichen Geschenk). Er zählt
    als Betriebseinnahme wie ein Erlös; DATEV bucht ihn vom Privatkonto statt von der
    Bank. Bisher galt fest 0 € — der Gewinn war zu niedrig.
  - *Storno:* Das Storno-Datum liegt frühestens am letzten Zahlungseingang (die
    Erstattung stand sonst in der EÜR vor dem Zufluss, § 11 EStG). Storno-Entwurf und
    Angebotsumwandlung entstehen samt Nummer und Protokoll in einem Schritt.
  - *Aktivierte Kosten* lassen sich nicht mehr stornieren (die Anlage schrieb sonst
    auf einen stornierten Kauf weiter ab); eine Rückabwicklung ist ein Abgang der
    Anlage.
  - *Restore:* Ein älteres Backup öffnet ein inzwischen abgeschlossenes
    Geschäftsjahr nicht wieder — der Abschluss wird übernommen, Eintrag unter „Hinweise“.
    Ein nach dem Restore bestätigter Entwurf mit schon festgeschriebener Nummer wird
    nach jeder Änderung wieder vorgemerkt.
  - *„Datenbank sperren“* sperrt nach dem Abmelden immer zu Ende, auch wenn ein
    langes Backup oder ein Export die Datenbank länger als 60 s hält (bisher brach es
    dann still ab, und Abo-Läufe gingen weiter).
  - *„Passphrase ändern“* steht innerhalb der Antwortzeit fest (höchstens etwa 20 s
    Wartezeit, sonst „beschäftigt“ ohne Änderung) und wartet ein laufendes Backup ab,
    bevor es die Datenbankdatei tauscht. Meldet der Browser trotzdem eine
    Zeitüberschreitung, zeigt die Seite „Ergebnis unklar“ und behält das neue
    Passwort in den Feldern.
  - *Abhängigkeiten:* `futures-*` 0.3.34 (Rest der Dependabot-Rust-Gruppe).
- **Nachbesserung Technik (Lauf F6-fix, ADR 0039 Nr. 141):** Der ZAP-API-Scan
  (`security.yml`) bricht nicht mehr beim Erzeugen der OpenAPI-Beschreibung ab:
  `openapi_from_registry.py` nimmt alle Registry-Einträge (seit F6 genau die
  Allowlist); die CI erzeugt die Beschreibung jetzt bei jedem Lauf. Die
  App-Icon-Vorlage liegt unter `docs/assets/app-icon/` statt `docs/assets/logo/`,
  weil das Veröffentlichen Ordner namens `logo` auslässt und der PDF-Test im
  öffentlichen CI sonst fehlschlug. Die nur noch von Tests genutzten Desktop-Helfer
  `backup::unlock` und `backup::needs_onboarding` sind entfernt (Entsperren nur über
  `server::auth`).
- **Nachbesserung Fachlogik (Lauf F5-fix, ADR 0039 Nr. 137, ADR 0040 Nr. 61):**
  - Festschreiben übernimmt die Summen des ausgestellten Belegs (USt je
    Steuergruppe wie XML und PDF); ein Entwurf mit Summen aus der älteren
    Rundung bzw. aus einem älteren Backup weicht nicht mehr vom Beleg ab.
  - Abo-Vorlagen mit Steuerart E (§19) erzeugen nach dem Wechsel zur
    Regelbesteuerung keinen nummerierten Entwurf mehr, der beim Festschreiben
    scheitert: Hinweis „übersprungen“, keine Nummer; Speichern mit E bei
    Regelbesteuerung lehnt der Server ab.
  - DATEV-Stapel: Bei Geldbewegungen ist „Konto“ das Geldkonto und
    „Gegenkonto“ das Sachkonto mit BU-Schlüssel (Kosten, aktivierte Kosten,
    Storno-Erstattung und Privatentnahme im Haben); negative Anteile einer
    Steuergruppe werden gegengebucht statt weggelassen.
  - Paket-Provenienz auch an Angeboten und Abo-Vorlagen: eine bearbeitete
    Position verliert `source_package_*` schon beim Speichern.
  - Der Hinweis zur eingebauten Storno-Vorlage entsteht erst nach dem
    erfolgreichen Festschreiben.
  - Druck-PDF mit AGB: auch GoToR-, Thread-, Movie-, Sound-, RichMediaExecute-,
    SetOCGState- und GoTo3DView-Aktionen, Medien-/3D-Annotationen und Links
    außer http(s)/mailto werden entfernt.
  - Beträge über 1 Mrd. € bei Kosten, Zahlungen, Anschaffungen, Abgangserlösen,
    Privatbewegungen und Kosten-Abos sind ein Validierungsfehler statt eines
    Überlaufs.
  - Die Übersicht wechselt nach einem Ruhezustand des Geräts sofort auf den
    neuen Tag (Fokus, `pageshow`, Minutentakt).
  - Hinweis „Gültiger App-Code mit falschem Daten-Passwort“: Jede Folge hat
    eine eigene Kennung; zwei Folgen in derselben Sekunde (etwa um einen
    Neustart) verschluckten bisher den zweiten Hinweis.
- **Aktivierte Kosten doppelt in der EÜR (Lauf F5b):** Eine Kostenposition, aus
  der eine Anlage angelegt wurde, zählte zusätzlich zur AfA als Ausgabe. Jetzt
  wirkt sie nur über die AfA; der DATEV-Stapel bucht sie als Zugang aufs
  Anlagekonto. Abgeschlossene Jahre mit solchen Posten meldet der
  Snapshot-Abgleich.
- **Storno-Überhang in der Ausfüllhilfe:** Übersteigen Storno-Erstattungen die
  Einnahmen eines Jahres, steht der Überhang als Betriebsausgabe in der Hilfe
  (bisher als nicht einzutragende „Zeile 18“, die ELSTER für erstattete
  Umsatzsteuer nutzt).
- **DATEV-Konten:** Auffang „Sonstige betriebliche Aufwendungen“ 4900/6300
  (bisher SKR04 6800 = Porto); Anlagenverkauf ohne Umsatzsteuer (bisher
  19-%-Automatikkonto) mit Restbuchwert-Buchung; vollständige Windows-1252-
  Tabelle, Zeilenumbrüche in Texten brechen keine Buchungszeile mehr um.
- **EÜR-PDF und Deckblatt:** Datum als TT.MM.JJJJ; die Summe der Einnahmen-Tabelle
  zieht die aufgeführten Stornos ab.
- **E-Rechnungs-Empfang (Lauf F5c):** Abgeschnittenes oder unbalanciertes XML
  wird abgelehnt (KB-0015); UBL liest den Rechnungstyp (BT-3), Gutschriften
  werden in CII und UBL mit jedem Gutschrift-Code abgelehnt (bisher lief eine
  UBL-Gutschrift als Kosten durch).
- **Versandprotokoll (R4-S2-EMAILLOG):** Scheitert der Eintrag im
  `email_log` nach einem Versand, gibt es einen Audit-Eintrag
  `mail.log_failed`, einen Eintrag unter „Hinweise“ und eine Warnung nach dem Senden statt
  nur einer Log-Zeile.
- **Eigene SMTP-CA:** Eine CA-Datei ohne PEM-Zertifikat wird mit klarer Meldung
  abgelehnt (bisher scheiterte erst der Handshake mit „UnknownIssuer“).

- **Beträge und Mengen (Lauf F5a, ASVS V5.4.3):** Mengen bis 1 000 000 mit
  höchstens 4 Nachkommastellen, Einzelpreise bis 1 Mrd. €, Steuersätze bis
  100 %; Summen laufen nicht mehr still über (Release mit Überlaufprüfung).
  AfA und betrieblicher Anteil rechnen ganzzahlig, Fehlertexte ohne
  Gleitkomma.
- **Zahlungshistorie:** Eine beschädigte Historie führt zu einer Meldung statt
  zu fehlenden Einnahmen in der EÜR; eine Zahlung auf einen Altbestand ohne
  Historie übernimmt die frühere Zahlung.
- **Prüfungen:** Annahmedatum eines Angebots zwischen Angebotsdatum und heute;
  E-Mail-Adressen brauchen eine Top-Level-Domain mit mindestens zwei
  Buchstaben; der Abgang einer Anlage wird im fachlichen Kern geprüft.
- **§14c-Schutz schon beim Speichern:** Ein Entwurf mit Umsatzsteuer bei §19
  wird nicht mehr gespeichert; ein anonymisierter Kontakt bekommt keinen neuen
  Entwurf.
- **Restore-Tausch (Lauf F4d, ADR 0039 Nr. 133):** Verschwindet das WAL der
  Datenbank zwischen Prüfung und Verschieben (SQLite löscht es beim Schließen
  der letzten Verbindung nachlaufend), bricht der Tausch nicht mehr mit „nicht
  gefunden“ ab. Tests, die nach einem Festschreiben Backups zählen, warten das
  Lock-Backup im Hintergrund jetzt deterministisch ab.
- **Nachbesserung Backup (Lauf F3c, ADR 0039 Nr. 129, ADR 0040 Nr. 57–59):**
  - Das Backup vor einer Datenbank-Umstellung entsteht im Format, das die
    vorherige Version liest (beim Update von 2026.10 Format 2) — der
    dokumentierte Rückfall auf die alte Version mit genau diesem Backup
    funktioniert. Handbuch: vor einem Update mit der alten Version sichern.
  - „Datenbank sperren“ und das Herunterfahren schreiben ein noch ausstehendes
    Backup eines eben festgeschriebenen Belegs, bevor die Datenbank schließt
    (bisher fiel es dann bis zum nächsten Entsperren aus).
  - Wiederherstellung: Der entschlüsselte Inhalt wird vor dem
    Sicherheits-Backup ausgepackt und freigegeben; beides braucht den
    Arbeitsspeicher nacheinander statt gleichzeitig (bis etwa dreimal die
    Backup-Größe). Ein in der Prüfung abgelehntes Backup erzeugt kein
    Sicherheits-Backup mehr.
  - Herunterladen eines Backups aus einem Ordner-Ziel auf einem anderen
    Dateisystem ohne Kopie nach `/data/tmp`.
  - Ein Backup, das symbolische Links oder verschwundene Archivdateien nicht
    sichern konnte, meldet das im Backup-Protokoll und als Hinweis „Backup
    unvollständig“ statt still „ok“.
  - Off-Site-Status „veraltet“ auch auf der Seite „Backups“; auf der Übersicht
    als Warnung (ein gerade hinzugefügtes Ziel ist kein Ausfall).
  - Lock-Backups nehmen die Zeit aus der Uhr der Anwendung statt der
    Systemuhr (Tests mit fester Uhr).
  - Tests: Abhängigkeiten im Dev-/Test-Profil optimiert übersetzt (Argon2id,
    SQLite, Typst) — keine 30-s-Zeitüberschreitungen mehr unter Volllast.
- **Nachzügler F1/F2 (Lauf F3b):**
  - Nummernkreis-Hinweis nach einem Restore: „es fehlen keine Nummern“ nur noch
    bei genau gleichen Ständen. Stehen im Backup höhere Zähler als in der
    ersetzten Datenbank (etwa eine neue Instanz mit Testdaten) oder trägt ein
    festgeschriebener Beleg dieselbe Nummer wie ein anderer, kommt die
    dringende Anleitung zum Anheben.
  - Hinweis „Abo-Termine nach der Wiederherstellung“: vorgerückte Kosten-Abos
    stehen getrennt — ihre gebuchten Kosten sind verworfen und von Hand
    nachzutragen. Abgelaufene Vorlagen und Abos ohne Auto-Buchung bekommen
    keine Prüfung „Rückstand …“ mehr (blieb sonst für immer stehen), pausierte
    Reihen entscheidet man beim Fortsetzen.
  - Anlagen-Abgang in einer Transaktion: Datum vor der Anschaffung und ein
    später abgeschlossenes Jahr mit der Anlage im Bestand lehnen ab; offene
    AfA ab dem Abgangsjahr wird (mit Audit) durch die anteilige AfA bis zum
    Abgangsmonat ersetzt, statt nach dem Abgang dauerhaft doppelt zu zählen.
  - Nach einer gescheiterten Prüfung „nach der Umstellung“ sagen auch spätere
    Anmeldeversuche das, statt „Daten wurden nicht verändert“.
  - Angebot festschreiben, annehmen und ablehnen nie parallel zu anderen
    Statuswechseln desselben Angebots; der Empfänger wird nur in Entwürfe
    übernommen (ein zurückgezogenes Angebot eines anonymisierten Kontakts bekommt
    keine Personendaten zurück).
  - Anonymisierung: Kosten-Abos mit der Person als Lieferant verlieren ihre
    Bezeichnung und eine Buchungsvorlage, die den Namen nennt.
  - Rechnungs-Entwurf verwerfen: schlichte Aktion „Verwerfen“ in der
    Detail-Ansicht (Grund, Daten-Passwort und App-Code), verworfene Entwürfe
    zeigen Zeitpunkt und Grund statt „Rechnung ausstellen“; Handbuch korrigiert.
- **Backup und Restore (Lauf F3a):** Ein Backup, das schon beim Snapshot oder
  der Verschlüsselung scheitert, steht im Backup-Protokoll und erzeugt einen
  Hinweis. Versteckte Temp-Dateien gleichzeitiger Archiv-Schreibvorgänge lassen
  das Backup nicht mehr scheitern. Die Rotation löscht nur `.kbk`-Dateien im
  Backup-Ordner bzw. unter `/backup-targets`. Ein ungültiger Restore-Aufruf
  (ohne Quelle, ohne Passphrase) antwortet mit 400, ohne kurz alle Änderungen
  zu sperren. Beim Tausch nach einem Restore wird das WAL der bisherigen
  Datenbank mitgesichert statt gelöscht (ein Rückfall verliert keine
  festgeschriebenen Transaktionen). Kein Absturz mehr bei einem vergifteten
  Mutex in Backup-Sitzung und SFTP-Handshake.
- **Halbes Storno-Paar aus der Desktop-Zeit (Lauf F2c):** Gibt es zu einer
  offenen Rechnung schon einen festgeschriebenen Storno, vervollständigt
  „Stornieren“ das Paar mit Audit, statt einen zweiten Storno anzulegen und mit
  „UNIQUE constraint failed“ abzubrechen.
- **Restore-Prüfung im Log (Lauf F2c):** Verwaiste Verweise in einem
  hochgeladenen Backup stehen als „Restore-Prüfung eines Backups“ im Log, nicht
  als Fehler beim Entsperren; beim Aufräumen einer abgebrochenen Einrichtung
  erzeugen schon verschwundene `-wal`/`-shm`-Dateien keinen Fehler-Logeintrag.
- **Keine doppelten Belegnummern nach einem Restore:** Wird ein älteres Backup
  in dieselbe Instanz eingespielt, übernimmt Klein.Buch die höheren
  Zählerstände von vor der Wiederherstellung (Audit, Hinweis); bereits vergebene
  Rechnungs-, Storno- und Kostennummern werden nicht erneut vergeben. Für die
  Wiederherstellung in eine neue Instanz lässt sich ein Nummernkreis unter
  Einstellungen → Backups → Nummernkreise anheben (nur aufwärts, Begründung,
  Re-Auth).
- **Backups nach dem eingespielten Stand** stehen nach einem Restore wieder im
  Backup-Verlauf (wählbar, nie automatisch gelöscht); fehlende Dateien
  verschwinden aus der Liste.
- **SFTP-Historie:** Die Rotation entfernt keine SFTP-Einträge mehr samt
  Audit „prune“, ohne dass die Datei gelöscht wurde.
- **Archiv-Prüfung:** Ein gescheiterter Monats-Vollscan läuft nicht mehr alle
  5 Minuten neu (6 h Wartezeit, Hinweis), Monats-Cron und „Jetzt prüfen“ laufen
  nie gleichzeitig („läuft bereits“), Sperren und Herunterfahren brechen den
  Scan je Datei ab.
- **„Backup überfällig“** rechnet mit dem Berliner Datum des Backups (vorher
  zwischen 0 und 2 Uhr einen Tag zu früh).
- **Entsperren:** Scheitert der Pflicht-Audit-Eintrag, bleibt die Instanz
  gesperrt (vorher lief sie ohne Sitzung entsperrt weiter).
- **Kosten-Abos ohne Doppelbuchung:** Tick, „Alle fälligen buchen“ und „Buchen“
  laufen je Abo unter einer Sperre; Nummer, Beleg, Audit und Vorrücken des
  Stichtags sind eine Transaktion mit Compare-and-Swap. Ein fehlerhaftes Abo
  bricht den Lauf nicht mehr ab, das Backup für bereits gebuchte Belege entsteht
  trotzdem. Ein Stichtag im abgeschlossenen Geschäftsjahr ergibt einen Hinweis
  statt einer Warnung bei jedem Tick.
- **Abo-Rechnungen absturzfest:** Entwurf und Vorrücken der Vorlage sind eine
  Transaktion; bricht der Prozess vor dem Ausstellen oder Versand ab, setzt der
  nächste Lauf dort fort, statt eine zweite Rechnung mit neuer Nummer anzulegen.
  Bearbeiten und Pausieren nehmen die Vorlagen-Sperre; Vorlagen mit
  archiviertem/anonymisiertem Kunden oder USt-Ausweis trotz §19 erzeugen keinen
  nummerierten Entwurf mehr.
- **„Datenbank sperren“ und „Passphrase ändern“** warten höchstens 10 s auf
  einen laufenden Abo-Lauf (der Lauf hört nach dem aktuellen Termin auf);
  Sitzungen enden beim Sperren sofort.
- **Auto-AfA:** Eintrag, Restbuchwert und Audit sind eine Transaktion; ein
  Fehler erzeugt einen Hinweis und wartet 6 Stunden statt bei jedem Tick neu zu
  scheitern.
- **Restore eines älteren Backups (Lauf F, F1d):** Abo-Vorlagen und Kosten-Abos
  stellen einen nach dem Backup schon abgerechneten Zeitraum nicht noch einmal
  in Rechnung (nächste Termine nur vorwärts auf den Stand vor der
  Wiederherstellung, Audit, Hinweis); Reihen ohne bekannten Stand warten auf
  „Rückstand …“ (Nachholen nur als Entwurf, „ab heute“ auch ohne den heutigen
  Termin). Begonnene Abo-Rechnungen und Nachhol-Bestätigungen aus dem Backup
  gelten nicht mehr. Ein wiederhergestellter Entwurf, dessen Nummer vor dem
  Restore schon festgeschrieben war, lässt sich erst nach dem bestätigten
  Abgleich mit dem Original festschreiben (Re-Auth, Begründung im Audit-Log).
  Der Hinweis „Nummernkreise“ unterscheidet „keine Lücke“ von der Anleitung für
  eine neue Instanz.
- **Nachhol-Bestätigung** gilt nur für Termine bis zu ihrem Tag; spätere folgen
  wieder der Grenze von drei. Fehlerhinweise nennen den gescheiterten Termin.
- **Tages-Backup** holt einen verpassten Slot nach (Uhrzeit kurz vor
  Mitternacht, Anmeldung vor der Uhrzeit nach einer Sperre); ein beim Sperren
  oder Herunterfahren ausgefallenes Backup nach dem Festschreiben holt der
  erste Lauf nach dem Entsperren nach.
- **Integritätsprüfung und Rotation bei vorgestellter Uhr:** Ein zukünftig
  datierter Prüflauf löst keinen Vollscan bei jedem Tick mehr aus; zukünftig
  datierte Backups verdrängen keine echten Tagesstände.
- **Entsperren** hängt nicht mehr an einem beim Sperren noch laufenden Abo-Lauf.
  „Jetzt erstellen“ schließt einen abgebrochenen Termin derselben Vorlage zuerst
  ab. Eine gleichzeitige Anonymisierung lässt keinen Abo-Entwurf mit den alten
  Kundendaten mehr entstehen. Der Hinweis „Instanz war N Tage gesperrt“ kommt
  auch, solange die Pause nach einem Restore besteht.
- **Anzeige:** Abgelaufene Abo-Vorlagen gelten nicht mehr als „überfällig“
  (Badge „beendet“, auch in der Übersicht); das Ergebnis je Backup-Ziel steht
  in Berliner Zeit statt roh.
- **Standard-Zahlungskonten** entstehen nicht mehr doppelt, wenn die Seite in
  zwei Tabs geöffnet wird.
- **Fälligkeiten nach Berliner Datum:** „heute fällig“, „in N Tagen“,
  „überfällig“ und der Zähler „fällig“ der Abo-Listen sowie die überfälligen
  Tage und das Alter des letzten Off-Site-Backups in der Übersicht rechnen mit
  dem Berliner Kalender statt mit der Uhr des Geräts (ein Backup von gestern
  23 Uhr heißt am Morgen „Gestern“, nicht „Heute“).
- **Datumsanzeige:** Reine Daten erschienen auf Geräten westlich von UTC einen
  Tag zu früh, Zeitstempel zwischen 0 und 2 Uhr mit dem Vortag, und ob ein
  Zeitstempel der Datenbank überhaupt lesbar war, hing vom Browser ab (WebKit).
  Ungültige Werte zeigen „—“ statt eines Fehlers.
- **`.neu`-Dateien** kamen nach dem Löschen bei jedem Start mit einem neuen
  Hinweis wieder.
- **Neuere Datenbank beim Entsperren:** Läuft ein älteres Image gegen eine
  Datenbank einer neueren Version, nennt die Meldung das passende Image statt
  eines Migrationsfehlers; die Instanz bleibt wie bisher gesperrt.

- **Storno-Absicherung:** Ein liegengebliebener Storno-Entwurf (nach einem
  gescheiterten Festschreiben) lässt sich nicht mehr über „Rechnung ausstellen“
  ohne das Original festschreiben — sonst wäre ein zweiter Storno möglich
  gewesen und die EÜR hätte die Erstattung doppelt gezählt. „Stornieren“
  verwendet einen solchen Entwurf wieder statt eine neue Nummer zu ziehen.
  Storno-Entwürfe entstehen nur noch über „Stornieren“, und ein Entwurf mit
  Datum in einem abgeschlossenen Geschäftsjahr wird abgelehnt (dort entsteht
  keine Nummer mehr).
- **Storno-Mail:** Stornorechnungen gehen mit eigenem Text raus (Nummer und
  Datum der stornierten Rechnung, kein negativer Betrag); eigene Fassung per
  `/inputs/mail-templates/storno-de.txt`. Storno-PDFs enden mit „Dieser Beleg
  storniert die genannte Rechnung vollständig.“ statt „Vielen Dank für Ihren
  Auftrag.“
- **Release-Lauf:** Eine Wiederholung ersetzt Quellarchiv und Notes nur, wenn
  zum früheren Build nie ein Image veröffentlicht wurde; Registry-Fehler gelten
  nicht mehr als „Image fehlt“. Der Prüfbefehl für die Signatur (README,
  `SECURITY.md`, Handbuch) erkennt nur noch den Release-Workflow auf Tags.
- **Stornieren mit echtem Validator:** Stornos scheiterten mit dem KoSIT-Validator
  des Images an BR-27 („Einzelpreis darf nicht negativ sein“): Das Storno-XML
  trug negative Einzelpreise, das Festschreiben brach ab. Jetzt steht das
  Vorzeichen in der Menge (BT-129), der Preis ist positiv, Zeilen- und
  Gesamtbeträge bleiben negativ. Die Abnahme gegen das Image storniert jetzt
  eine Rechnung mit dem echten Validator.
- **E-Rechnung des Stornos:** Der Verweis auf die stornierte Rechnung (BT-25)
  enthielt die interne Datenbank-ID statt der Rechnungsnummer; jetzt stehen
  Nummer (BT-25) und Rechnungsdatum (BT-26) des Originals im XML. Bereits
  festgeschriebene Storno-XML bleiben unverändert (das PDF nennt die Nummer).

- **Gelegentlicher Fehler „Datei nicht gefunden“** beim Passphrase-Wechsel, bei
  der Umstellung auf die verschlüsselte DB und beim Restore-Tausch: SQLite
  entfernt `-wal`/`-shm` beim Schließen selbst; Klein.Buch prüfte erst und
  löschte dann. Jetzt wird direkt gelöscht und „fehlt schon“ als Erfolg gewertet.

### Removed

- Testrest `sandbox-write-test.txt`.
- Repo-Funktion `invoices::mark_canceled` ohne Idempotenz-Prüfung (nur noch von
  Tests genutzt; das Storno-Paar läuft über `lock_with_pair_cancel`).
- **Desktop-/Tauri-Altcode (Lauf F6, ADR 0039 Nr. 138):** die 18 im Server nur
  gesperrt mitgeführten Commands samt Kern (Explorer/Öffnen, Restore-Vorschau per
  Pfad, altes Einzel-Backup-Ziel, Vorgänger der Auth-Routen, Lizenzpfad,
  `mail_oauth_*`), `Route::Replaced/Dropped/Blocked`, `DesktopActions` im
  Anwendungskontext, der Microsoft-Graph-Versand (`mail::oauth_ms` samt Test),
  die Lizenz-HTML der Desktop-Fassung im Handbuch-Ordner (die vollständige Liste
  steht unter `/legal/`), die Desktop-Icons unter `src-tauri/icons/` (Logo-Vorlage
  jetzt unter `docs/assets/app-icon/`), die Vorlagen-Logos `static/{tauri,vite,svelte}.svg`
  und die doppelten Leser `quotes_attachments_list` und `fiscal_year_auto_close_get`.
  Ein aus der Desktop-Fassung übernommenes Microsoft-Konto versendet nicht (Hinweis
  auf SMTP) und verliert beim Umstellen auf SMTP Token und OAuth-Angaben.
- `svelte.config.js` (SvelteKit 3 liest nur noch `vite.config.js`) und die
  Build-Freigabe für esbuild in `pnpm-workspace.yaml` (nicht mehr im Baum).

## [2026.10.0] — Web-/Docker-Fassung, veröffentlicht 2026-10-24

Erstes Release als Webapp im öffentlichen Docker-Image
`ghcr.io/hozwei/klein-buch:2026.10.0` (Phase W, ADR 0039): ein Server
(`klein-buch-server`) mit Bedienung im Browser, Anmeldung mit Passphrase und TOTP,
verschlüsselte Datenbank, Backups auf mehrere externe Ziele (Verzeichnis, SFTP),
Responsive-Oberfläche für Smartphone, Tablet und Desktop. Die Windows-Desktop-App
(Tauri) entfällt; einen Datenumzug aus v2026.5.x gibt es nicht (E17). Image
signiert (cosign), mit SBOM und Quellarchiv der Komponenten Dritter am Release.

### Fixed

- **Sicherheit (W14):** Anmeldung lässt sich nicht mehr durch fremde
  Fehlversuche dauerhaft blockieren (Einmal-Code kommt immer durch,
  Gerätecookie wird erneuert); langsam gesendete Anfragen belegen den Server
  nicht mehr; die Bestätigung mit Passphrase und Code (Re-Auth) erneuert die
  Sitzung, ein gestohlenes Sitzungs-Cookie verliert damit seine Rechte; das
  Ändern von Bankverbindung, Zahlungskonten und §19-Status, das Anonymisieren
  und der Geschäftsjahres-Abschluss verlangen diese Bestätigung; Vorlagennamen
  können keine Pfade mehr sein; Fehlerantworten werden nicht mehr dauerhaft im
  Browser zwischengespeichert; präparierte ZUGFeRD-PDFs (Dekompressionsbombe)
  werden abgewiesen.
- **Storno-Datum:** muss zwischen Rechnungsdatum und heute in einem offenen
  Geschäftsjahr liegen; vorher konnte ein Storno die EÜR eines bereits
  abgeschlossenen Jahres ändern. (W14)

- **Rechnung ohne Fälligkeitsdatum:** Der echte Validator lehnt eine
  E-Rechnung mit offenem Betrag ohne Fälligkeitsdatum ab (BR-CO-25). Das
  Festschreiben meldet das jetzt vorab mit klarer Meldung. (W11)
- **XRechnung von KoSIT abgelehnt (Schemafehler):** In der Notiz mit der
  §19-Klausel stand der Code `REG` vor dem Text; das CII-Schema verlangt die
  umgekehrte Reihenfolge. Mit dem echten Validator ließ sich deshalb keine
  §19-Rechnung festschreiben. Jetzt Text vor Code; echte Validator-Tests
  (`sidecar_real`) sichern das ab. (W11)
- **Sporadischer Absturz beim Beenden:** Das von SQLCipher genutzte OpenSSL
  räumte sich beim Prozessende per `atexit` auf, während Hintergrund-Threads
  noch Datenbankverbindungen schlossen („double free or corruption“, in CI
  aufgefallen). OpenSSL wird jetzt ohne dieses Aufräumen initialisiert. (W10)
- **Zeitstempel um 1–2 Stunden zu früh:** Bei einer Prozess-Zeitzone ungleich
  UTC (z. B. `TZ=Europe/Berlin`; vermutlich auch die Desktop-Fassung) lagen
  alle von der Datenbank gesetzten Zeitstempel — Audit-Log, Backup- und
  E-Mail-Protokoll, Anlage- und Änderungszeiten — um den Zeitzonen-Versatz zu
  früh (SQLite `datetime('now','utc')`). Der Server rechnet jetzt intern immer
  in UTC; die Datumsfilter der Protokolle gelten in Europe/Berlin. Bereits
  gespeicherte Zeitstempel bleiben unverändert. (W9)
- **Logo-Dateiname vom Browser:** Das Profil übernahm den Logo-Dateinamen vom
  Browser; darüber ließen sich mit einer Sitzung beliebige Dateien lesen oder
  löschen. Jetzt setzt nur der Logo-Upload den Namen, und nur `logo.*`/
  `signature.*` im Branding-Ordner werden gelesen. (W7)
- **Restore-Protokoll:** Der Audit-Eintrag zum Restore ging verloren (er stand
  in der beim Tausch verworfenen Datenbank) bzw. hing an einem Neustart. Jetzt
  steht „Restore angefordert“ im Sicherheits-Backup und „Restore angewendet“
  in der wiederhergestellten Datenbank, auch nach Neustarts. (W7)
- **Backups trugen immer die aktuelle Schema-Version** statt der der
  gesicherten Datenbank (relevant für Backups vor einer Umstellung). (W7)
- **SFTP-Backups** wurden direkt unter dem Endnamen geschrieben; ein Abbruch
  hinterließ eine halbe Datei. Jetzt Temp-Datei + Umbenennen. (W7)
- **E-Rechnung (XRechnung) wurde von KoSIT abgelehnt**, wenn keine USt-IdNr.
  hinterlegt war: Es fehlten die Gruppe „Verkäufer-Kontakt“ (BG-6, BR-DE-2) und
  eine Verkäufer-Kennung (BR-CO-26). Jetzt enthält das XML BG-6 (Ansprechpartner
  = Inhaber-Name bzw. Firmenname, Telefon, E-Mail) und ohne USt-IdNr. die
  Steuernummer als BT-29. Ohne Telefonnummer in den Stammdaten wird nicht
  festgeschrieben (klare Meldung statt KoSIT-Fehler). KoSIT-Ablehnungen nennen
  die verletzten Regeln, statt nur „validator exit-code 1“. (W1)
- **Belegdatum und Geschäftsjahr in Europe/Berlin:** Das Frontend nahm „heute“
  in UTC (zwischen 0 und 1–2 Uhr der Vortag, in der Neujahrsnacht eine Nummer
  aus dem Vorjahreskreis); der Server nutzte die Prozess-Zeitzone. Jetzt gilt
  überall Europe/Berlin (`chrono-tz`, `src/lib/date.ts`). (W1)
- **Geschäftsjahr kommt vom Server:** Rechnung, Angebot, Kosten, Kosten aus
  E-Rechnung und Privatbewegung leiten das GJ aus dem Belegdatum ab; ein
  abweichendes GJ vom Client wird abgelehnt. (W1)
- **Abo-Rechnungen ohne Doppel:** Vorrücken per Compare-and-Swap plus Sperre je
  Vorlage zwischen automatischem Lauf und „Jetzt erstellen“. (W1)
- **Logo und Unterschrift im Backup:** Backups (Format 2) enthalten
  `data/branding`; der Restore spielt es zurück. Backups im Format 1 lassen sich
  weiter einspielen, das gespeicherte Logo bleibt dann unverändert (Hinweis im
  Restore-Ergebnis). (W1)
- **Typst-Escaping vollständig:** Jedes ASCII-Satzzeichen in Paket-Texten wird
  maskiert; `//`, `=`, `/ …:`, `~` oder `--` verschwanden sonst im PDF oder wurden
  umgedeutet. (W1)
- **Sidecar-Mock nur in Debug-/Test-Builds;** ein Release-Build mit gesetztem
  `KLEIN_BUCH_SIDECAR_MOCK` startet nicht. (W1)
- **Integritätsprüfung** zählt den Monat in Europe/Berlin (lief am Monatsersten
  sonst doppelt oder gar nicht). (W1)
- **Archiv-Schreibpfad:** eindeutige Temp-Datei, Schreibschutz vor dem
  Sichtbarwerden; ein abgebrochenes Festschreiben hinterlässt keine Datei mehr,
  die jeden weiteren Versuch mit „existiert mit anderem Hash“ blockiert. (W2)
- **Integritätsprüfung:** nicht lesbare Dateien (z. B. Rechte) gelten nicht mehr
  als manipuliert, sondern bekommen einen eigenen Hinweis. (W2)
- **Backup konsistent:** DB-Snapshot per `VACUUM INTO` über eine eigene
  Verbindung (auch bei parallelem Schreiber ein sauberer Stand), verschlüsselt
  wie das Original; Backups laufen nacheinander, Dateinamen mit Millisekunden
  (zwei Backups in derselben Sekunde überschrieben sich); Off-Site-Kopien per
  Temp-Datei + fsync + Umbenennen. (W2)
- **Tägliches Backup auch im laufenden Betrieb** (Scheduler), nicht nur beim
  Entsperren. (W2)
- **Kein `SQLITE_BUSY` bei parallelen Buchungen:** Schreib-Transaktionen mit
  `BEGIN IMMEDIATE`, `busy_timeout` 15 s. (W2)
- **Restore:** Archivdateien sind danach wieder schreibgeschützt. (W2)
- **Falsche Passphrase beim Entsperren** meldete bei verschlüsselter DB einen
  Datenbankfehler statt „falsches Passwort“: SQLCipher scheitert schon beim
  Verbindungsaufbau (`file is not a database`); das gilt jetzt als falsche
  Passphrase. (W5)
- **Mail-Empfänger und SMTP-Benutzer** stehen nur noch auf DEBUG im Log, nicht
  mehr auf INFO/WARN. (W5)
- **Restore-Tausch abbruchsicher:** eine Zustandsdatei steuert nach einem
  Abbruch das Zurückholen der Originale; ein Fehler in jeder Stufe stellt den
  alten Stand wieder her, nichts wird in unklaren Lagen gelöscht; solange ein
  Restore offen ist, sind Einrichten und Entsperren gesperrt. Der Marker
  entsteht erst nach dem Audit-Eintrag; Dateinamen mit `..` blockieren den
  Restore nicht mehr; kopierte Dateien werden mit `fsync` geschrieben. (W2)
- **Parallele Festschreib-Läufe** derselben Rechnung laufen nacheinander;
  Archivpfade sind gegen gleichzeitiges Schreiben gesperrt; nicht registrierte
  Dateien am Zielpfad werden beiseitegelegt statt gelöscht; eine Wiederholung
  nach Abbruch legt eine neue Dateiversion an, statt die Rechnungsnummer zu
  blockieren. (W2)
- **Passphrase nie im Log:** Statement-Logging von sqlx für die
  verschlüsselten Verbindungen abgeschaltet (sonst konnte `PRAGMA key` als
  „langsames Statement“ im Log landen). (W2)
- **Migrations-Export** blockiert keine Schreiber mehr und hinterlässt bei
  Fehlern keine offene Transaktion. (W2)

### Changed (Umbau)

- **Kern-Schnitt (W4):** Die 183 Commands sind als Kerne
  `commands::<bereich>::<name>(ctx, args)` von Tauri getrennt: gemeinsamer
  Anwendungskontext `AppCtx` (Pfade, schließbarer DB-Pool, Sitzung,
  Restore-Audit, Benachrichtigungen, Zugangsdaten), Args-Structs mit den
  bisherigen camelCase-Schlüsseln, Registry aller Kerne mit Metadaten für den
  späteren HTTP-Dispatcher; die Tauri-App nutzt bis zum Umbau generierte dünne
  Wrapper. Pfade für den Server kommen aus `KLEIN_BUCH_DATA_DIR`,
  `KLEIN_BUCH_INPUTS_DIR`, `KLEIN_BUCH_RESOURCE_DIR`, `KLEIN_BUCH_WEB_DIR`
  (Pflicht); `inputs/` wird bei jedem Start ergänzt, nie überschrieben. Der
  ungenutzte Command `invoices_archive_paths` entfällt.
- Neue Tests: Storno über den Kern, §19-Prüfung in der Vorlagen-Vorschau,
  UTF-8-Prüfung beim Rechnungseingang, Restore-Audit über den Kontext,
  Registry = Frontend-Aufrufe, kein Tauri-Bezug außerhalb der Hülle.
- **Tauri entfernt, HTTP-Server `klein-buch-server` (W5):** axum-Router, hyper
  mit Header-Read-Timeout 10 s, einziger Einstieg (Subcommands `serve`,
  `healthcheck`, `version`; Adresse `KLEIN_BUCH_LISTEN`, Standard
  `0.0.0.0:8080`). Tauri samt Plugins, `build.rs`, `capabilities/` und
  `tauri.conf.json` entfallen; die Crate ist nur noch eine Bibliothek plus Binary.
- **`POST /api/cmd/{name}` mit verbindlicher Allowlist (W5):** 160 der 183 Kerne
  sind erreichbar; Einrichten/Anmelden laufen über `/api/auth/*`, Upload- und
  Download-Wege folgen in W7, Factory Reset, Drop-Folder und Microsoft 365 sind
  im Server gesperrt, Desktop-Funktionen („Im Ordner zeigen“) entfallen. Alles
  andere → 404, ohne Sitzung → 401, gesperrte DB → 423, zu großer Körper → 413,
  ohne Pflicht-Header `X-KleinBuch-Request` → 403.
- **Fehler mit Status statt String (W5):** fachliche Fehler 4xx mit Meldung,
  Trigger-Verstöße 409 mit Text, Sidecar-Ausfall 503, interne Fehler 500 nur mit
  Referenz (voller Text im Log).
- **Einrichten und Anmelden (W5, vorläufig):** `GET /api/auth/status`,
  `POST /api/auth/setup`, `POST /api/auth/login` mit Passphrase; Sitzungs-Cookie
  `__Host-kb_session` (HttpOnly, Secure, SameSite=Strict). TOTP, Setup-Token und
  Rate-Limit folgen in W6.
- **Start fail-closed (W5):** Volumes `/data` und `/inputs` müssen eingebunden
  und beschreibbar sein; Instanz-Marker `/data/.klein-buch-instance` und DB
  müssen zusammenpassen; keine unverschlüsselte DB; Selbsttest des
  Schreibschutzes im Archiv; Reste in `/data/tmp` werden entfernt — alles vor dem
  Port-Bind. Ein vorgemerkter Factory Reset wird nicht angewendet, sondern
  verweigert den Start (E7); eine Sperrdatei verhindert einen zweiten Prozess
  auf demselben Datenverzeichnis. Beim Entsperren prüft eine Stichprobe, ob die
  Archivpfade zum Datenverzeichnis passen, bevor Scheduler und Pool aktiv werden.
- **Jobs und Idempotenz (W5):** Festschreiben, Storno, Versand, Backup, Restore
  und Exporte laufen als Job (`202` + `GET /api/jobs/{id}`), ändernde Aufrufe als
  eigener Task mit Idempotenz-Schlüssel — ein abgebrochener Request hinterlässt
  nichts halb, die Wiederholung liefert dasselbe Ergebnis. Während eines Restores
  sind Änderungen gesperrt und der Scheduler pausiert; danach fährt der Server
  geordnet herunter (wartet auf laufende Tasks), der Neustart wendet ihn an.
  Einrichten und Anmelden laufen ebenfalls abbruchsicher.
- **Sicherheits-Header und Lastgrenzen (W5):** CSP mit Hashes der
  Inline-Skripte (kein `unsafe-inline` für Skripte), `nosniff`,
  `Referrer-Policy`, COOP/CORP, `Permissions-Policy`, `Cache-Control` je
  Routenklasse; höchstens 64 Anfragen gleichzeitig, je Kostenklasse 2/2/1/1
  Plätze, 30 Render-/Java-Aufrufe je Minute und Sitzung, 30-s-Timeout.
- **Logging ohne Geheimnisse (W5):** `sqlx::query=off` fest im Filter,
  Standard `info`; das Zugriffsprotokoll nennt Methode, Pfad ohne Query,
  Status und Dauer.
- **`/healthz` (W5):** nach außen 200/503 ohne Körper, über Loopback mit
  `{"state":"setup|locked|unlocked"}`; `locked` gilt als gesund.
- **Frontend über HTTP (W5):** `src/lib/transport.ts` ersetzt die
  Tauri-Bridge (Pflicht-Header, Idempotenz-Schlüssel, Job-Abfrage, Anmeldung bei
  401/423, Warten auf den Neustart nach dem Restore).
- Neue Tests: Allowlist, Routen-Inventar, Statustabelle aller Fehlervarianten,
  Header je Routenklasse, Fail-closed-Start (auch über das Binary), Abbruch
  beim Festschreiben mit Wiederholung, Header-Timeout, parallele Vorschauen,
  keine Geheimnisse im Log, Healthcheck-Subcommand; Vitest für den Transport.
- **Anmeldung mit zweitem Faktor (W6):** Einrichtung mit Setup-Token aus dem
  Container-Log, Daten-Passwort und TOTP (QR-Code, lokal erzeugt); zehn
  Einmal-Codes für den Notfall. Anmeldung mit Passwort und App-Code (der Code
  wird vor dem Passwort geprüft, beide Fehler antworten gleich). Ein Code gilt
  nur einmal, auch über Neustarts. `klein-buch-server totp-reset` setzt den
  zweiten Faktor zurück (Neueinrichtung mit Setup-Token und Passwort).
- **Sitzungen (W6):** Cookie `__Host-kb_session` (HttpOnly, Secure,
  SameSite=Strict), neue ID bei jeder Anmeldung, Ablauf nach 30 min ohne Nutzung
  bzw. nach 12 h, Abmelden, „Datenbank sperren“ (schließt die Datenbank, meldet
  alle Geräte ab, pausiert geplante Aufgaben).
- **Re-Auth für kritische Aktionen (W6):** Restore, Backup-Ziele, Mailkonten,
  Steuerberater-, Detail-, Migrations- und DSGVO-Export, TOTP und Einmal-Codes
  erneuern, Passwort ändern verlangen eine Bestätigung mit Passwort und
  App-Code (gilt 5 min); der Dialog erscheint automatisch.
- **Daten-Passwort ändern (W6):** Die Datenbank wird mit dem neuen Passwort neu
  verschlüsselt (Prüfung vor dem Tausch), andere Geräte werden abgemeldet,
  danach entsteht ein Backup. Ältere Backups bleiben mit dem alten Passwort
  einspielbar.
- **Schutz vor Durchprobieren (W6):** Wartezeiten je Gerät bzw. IP (IPv6 je
  /64) ab dem fünften Fehlversuch, verdoppelnd bis 1 h, keine Dauersperre;
  ein globales Budget für Clients ohne Gerätecookie. Nach der Anmeldung
  erscheint die Zahl der Fehlversuche seither.
- **Betrieb hinter dem Reverse-Proxy (W6):** `KLEIN_BUCH_PUBLIC_URL` ist
  Pflicht (falscher Host → 421, fremder Origin → 403);
  `KLEIN_BUCH_TRUSTED_PROXIES` legt fest, von wem `X-Forwarded-*` gilt.
  Anmelden nur über HTTPS (Proxy), direkt am Server oder mit
  `KLEIN_BUCH_INSECURE_HTTP=1` (nur Tests). HSTS bei HTTPS.
- Entfernt: der Übergangspfad für unverschlüsselte Bestands-Datenbanken (E17).
- Neue Tests: Setup-Token, TOTP vor der KDF, Replay auch nach Neustart,
  Einmal-Codes, `totp-reset`, Wartezeiten je IP/Gerät, Proxy-Header, Host/Origin,
  HTTPS-Regel, HSTS, Re-Auth über alle 13 betroffenen Commands, Sperren,
  Abmelden, Passwort-Wechsel mit Datenerhalt, Log ohne Geheimnisse.
- **Downloads statt Speicherort (W7):** EÜR-PDF, ELSTER-CSV, Einzelaufstellung,
  DATEV-Stapel, Steuerberater-Paket, Migrations-Export und DSGVO-Auskunft
  entstehen auf dem Server und kommen als Browser-Download (einmal abrufbar,
  nach 1 h gelöscht); kein Pfad mehr vom Browser.
- **PDF-Vorschau im Browser (W7):** Vorlagen-, Paket- und Broschüren-Vorschau,
  Angebots-Bundle sowie Rechnungs- und Angebots-PDFs öffnen in einem
  Vorschau-Dialog (mit „In neuem Tab öffnen“ und „Herunterladen“).
- **Archivdateien hash-geprüft (W7):** Jeder Abruf prüft SHA-256 und wird
  protokolliert (`archive.read`); eine veränderte Datei wird nicht ausgeliefert.
  Belege, Anhänge und andere fremde Dateien kommen nur als Download.
- **Uploads (W7):** Belege, Anhänge, Verträge, Rechtsdokumente, E-Rechnungen,
  Logo und Unterschrift werden gestreamt hochgeladen; Größe je Art (20 MiB,
  E-Rechnung 10 MiB, Logo/Unterschrift 2 MiB), Typ am Inhalt geprüft (SVG nur
  für Logo/Unterschrift, kein HTML), zu wenig freier Platz wird abgelehnt
  (`KLEIN_BUCH_MIN_FREE_BYTES`).
- Entfernt: „Im Ordner zeigen“, „Ordner öffnen“ und „Pfad kopieren“.
- **Restore per Upload oder aus der Liste (W7):** Backup-Datei hochladen (mit
  Fortschritt) oder ein lokales Backup auswählen; eingespielt werden Backups ab
  v2026.5 (Schema 30). Erst wird entschlüsselt, dann das Sicherheits-Backup
  angelegt — eine falsche Passphrase verändert nichts. Das Entpacken ist gegen
  Zip-Bomben, fremde Pfade und Symlinks gehärtet; fehlende eigene PDF-Vorlagen
  meldet danach die Inbox.
- **Pflicht-Backup vor jeder Datenbank-Umstellung (W7):** Beim Entsperren einer
  älteren Datenbank entsteht vorher ein geprüftes Backup; ohne Backup keine
  Umstellung.
- **Mehrere externe Backup-Ziele (W7):** Ordner unter `/backup-targets` (eigene
  Volumes) und SFTP-Server, beliebig kombiniert; jedes Backup geht an jedes Ziel,
  Fehler je Ziel im Protokoll und in der Inbox, Status in der Übersicht und im
  Healthcheck. Der Cloud-Ordner-Vorschlag entfällt. Neue Variablen:
  `KLEIN_BUCH_BACKUP_TARGETS_DIR` (Standard `/backup-targets`),
  `KLEIN_BUCH_MAX_RESTORE_BYTES` (Standard 4 GiB), `KLEIN_BUCH_MIN_FREE_BYTES`
  (Standard 2 GiB).
- **Zugangsdaten in der verschlüsselten Datenbank (W8):** SMTP- und
  SFTP-Passwörter liegen in einer eigenen Tabelle der SQLCipher-Datenbank
  (Migration 0031, Schema 31) statt im Schlüsselbund des Betriebssystems; nie im
  Migrations-Export, in der DSGVO-Auskunft, in Protokollen oder im Audit. Die
  Abhängigkeiten `keyring` und `libdbus` entfallen.
- **Passwort gilt nur für Server, Port und Benutzer (W8):** Ändert sich eins
  davon, wird das gespeicherte Passwort gelöscht, mit Benutzer ist ein neues
  Pflicht. Gespeicherte Postfächer und SFTP-Ziele testet „Testen“ in der Liste
  (nur per ID); Ad-hoc-Tests brauchen das eingegebene Passwort.
- **Verbindungstests begrenzt (W8):** nur freigegebene Ports (SMTP 25, 465, 587,
  2525; SFTP 22, 2222; anpassbar über `KLEIN_BUCH_ALLOWED_TEST_PORTS`), höchstens
  10 Tests je Minute, eine einheitliche Fehlermeldung. Ein SFTP-Ziel übernimmt
  nur den Host-Fingerprint aus dem letzten Test dieses Servers.
- **SMTP (W8):** unverschlüsselt nur ohne Benutzer und Passwort; EHLO-Name über
  `KLEIN_BUCH_SMTP_HELO_NAME`, eigene CA über `KLEIN_BUCH_SMTP_CA_FILE` (PEM-Datei
  im Volume). Die Microsoft-Anmeldung (Graph) ist im Server ausgeblendet;
  Microsoft-Konten lassen sich auf SMTP umstellen.
- **Desktop-Reste entfernt (W9):** Tauri-Pakete (`@tauri-apps/*`), Seiten
  „Zurücksetzen“ und „Rechnungs-Eingang (Ordner)“, der Kanal
  „System-Mitteilung“ bei den Erinnerungen; der Über-Dialog nutzt normale Links
  und zeigt die Quell-URL der Instanz.
- **Geschäftsjahr nur noch vom Server (W9):** Die Oberfläche schickt beim
  Anlegen von Rechnung, Angebot, Kosten und Privatbewegung kein Geschäftsjahr
  mehr mit; ein mitgeschickter Wert wird ignoriert.
- **Ohne Secure Context (W9):** Zwischenablage und zufällige IDs haben einen
  Rückfall, falls der Browser die Seite nicht als sicher einstuft (z. B. HTTP
  mit `KLEIN_BUCH_INSECURE_HTTP=1` in Tests). Entwicklung mit `pnpm dev` über
  einen Proxy auf den lokalen Server.
- Backup-Protokoll: Auslöser „Täglich (automatisch)“ statt „Täglich
  (App-Start)“ (der Tageslauf kommt seit W2 aus dem Scheduler). (W9)
- **Docker-Image (W11):** `Dockerfile` (Node → Rust → jlink-JRE → Debian
  trixie-slim, Basis-Images mit Digest, ohne Root, Healthcheck) und
  `compose.example.yaml` mit gehärteten Laufzeit-Einstellungen. Der Java-Sidecar
  liegt unter `src-tauri/sidecar/` (KoSIT-Validator 1.6.2, Mustang 2.23.0,
  XRechnung-Konfiguration 2026-01-31, Quellen und SHA-256 in `versions.json`,
  in CI geprüft); die Windows-Laufzeit ist entfernt.
- **Vorlagen in `/inputs` bleiben aktuell (W11):** Ein Seed-Manifest ersetzt
  unveränderte mitgelieferte Dateien durch neue Fassungen; vom Nutzer geänderte
  bleiben unangetastet, die neue Fassung liegt daneben als `<name>.neu`
  (Hinweis in der Inbox).
- Entfernt: Factory Reset und Rechnungs-Eingangsordner samt Code (seit W5 im
  Server gesperrt), Windows-Release-Workflow. (W11)
- **Auf dem Smartphone bedienbar (W10):** Navigation als ausklappbares Menü,
  Listen und Positionen als Karten, einspaltige Formulare, größere Schaltflächen
  (mind. 44 px), Meldungen unten. PDFs öffnen auf dem Handy in der PDF-Ansicht
  des Browsers; Belege lassen sich direkt mit der Kamera fotografieren. Geprüft
  auf allen Seiten bei 390, 768 und 1280 px Breite.
- **Lizenzen und Quellen unter `/legal/` (W12):** Die Lizenzseite entsteht beim
  Image-Build aus den tatsächlich eingebauten Teilen (Rust per cargo-about,
  mitgelieferte NOTICE-Dateien inkl. SQLCipher und OpenSSL, eingebettete
  Schriften, Java-Sidecar mit Inventar und Hinweisen, JS-Pakete des Bundles)
  und ist ohne Anmeldung erreichbar, ebenso `LICENSE.txt`, „Quellen Dritter“
  und die Debian-Paketliste; dieselben Dateien liegen im Image unter
  `/usr/share/doc/klein-buch`.
- **Quellangabe nach AGPL §13 (W12):** Anmeldung, Einrichtung und Über-Dialog
  zeigen Version und Commit mit Link auf genau diesen Quellstand (bei einem
  Release auf das Release); ein Build ohne Commit heißt
  „Entwicklungs-Build, Quellstand unbekannt“. Betreiber mit eigenen Änderungen
  setzen `KLEIN_BUCH_SOURCE_URL`.
- **Versionsprüfung (W12, opt-in):** Unter Einstellungen → Sicherheit &
  Anmeldung lässt sich eine tägliche Abfrage der Releases einschalten; eine
  neue Version erscheint als Hinweis in der Inbox. Standard aus — ohne
  Einschalten baut Klein.Buch keine Verbindung auf.
- **Release-Image (W12):** Signiertes Image mit SBOM, Schwachstellen-Scan und
  Quellarchiv „Quellen Dritter“ je Release; Lieferketten-Prüfungen in CI
  (cargo-deny, cargo-audit, pnpm audit, gitleaks), Actions per Commit-SHA.
- Ausgeblendet: die wirkungslosen Erinnerungsregeln des entfernten
  Rechnungs-Eingangsordners; ein falscher Code bei der Einrichtung des zweiten
  Faktors erscheint jetzt ebenfalls als Fehlversuch im Log (fail2ban). (W13)
- **Abhängigkeiten (W12):** russh 0.64, quick-xml 0.42, lopdf 0.46 und weitere
  Updates wegen Sicherheitsmeldungen (u. a. rustls, tokio). Der
  E-Rechnungs-Parser setzt Texte mit Entity-Referenzen (`&amp;`) jetzt am
  Element-Ende zusammen und liest auch CDATA-Abschnitte. ZIP-Dateien nur noch
  mit Deflate (Backups und Exporte von Klein.Buch nutzen nichts anderes).

### Documentation

- **ADR 0039 „Web-/Docker-Betrieb“:** Entscheidungen E1–E17, Bedrohungsmodell
  für den Internetbetrieb, Standardentscheidungen und die Entscheidungen
  während der Umsetzung; löst ADR 0002 (Tauri-Shell) und ADR 0038 (NSIS-Bundle)
  ab. Amendments in ADR 0001, 0008, 0009, 0011, 0013, 0014, 0023, 0028, 0034,
  0035, 0036, 0037; ADR-Index aktualisiert. (W3)
- `docs/architecture/security.md` überarbeitet (Threat Model für den
  Internetbetrieb, Stand der Umsetzung je Maßnahme, Doku-Drift bei Verifier und
  SQLCipher-Migration behoben, Write-once unter Linux ehrlich beschrieben);
  `gobd.md` zum neuen Archiv-Schreibpfad nachgezogen. (W3)
- **Handbuch und Betriebsdoku für den Docker-Betrieb (W13):** Neue Hilfe-Kategorie
  „Betrieb“ (Installation mit Docker Compose, Reverse-Proxy und TLS, Volumes
  und Rechte, zweiter Faktor, Updates und Rückfall, Notfall-Wiederherstellung,
  Speicher, Fehlersuche im Container mit fail2ban-Filter, Lizenz und
  Quellcode); alle übrigen Artikel auf Browser und Server umgestellt
  („Rechnungseingang per Upload“ statt Eingangsordner, Mail nur per SMTP,
  Downloads statt Speichern-Dialog); Artikel zu Zurücksetzen, Microsoft-Anmeldung
  und SmartScreen entfernt. `README.md` mit Schnellstart, neue `SECURITY.md`
  (Meldeweg, Signaturprüfung mit cosign) und `CONTRIBUTING.md`; Betriebs-,
  Architektur- und Build-Doku auf den Server-Betrieb umgestellt.

### Changed

- CI läuft auch auf dem Arbeitszweig `phase-w`, mit `concurrency`, nur
  Leserechten (`contents: read`), Node 22, pnpm-Version aus `packageManager`
  in `package.json`, festem Runner `ubuntu-24.04` und Zeitlimits. (W0)
- `LICENSE` enthält jetzt den vollständigen Text der AGPL-3.0 (vorher nur
  Urheberhinweis und Platzhalter); Urheberhinweis in `COPYRIGHT`, Hinweis zur
  Java-Laufzeit der Windows-Releases v2026.5.0/v2026.5.1 in
  `THIRD-PARTY-NOTICES.md` und `versions.json`. CI prüft die Prüfsumme von
  `LICENSE`. (W1)
- Migrationen `0001` und `0002` mit LF statt CRLF (sqlx prüft die Rohbytes);
  ein Test verhindert CR in Migrationen. (W1)
- Frontend-Tests mit vitest (`pnpm test`, läuft in CI). (W1)
- PDF-Erzeugung, Passphrase-Ableitung und ZIP laufen außerhalb der
  async-Runtime; gleichzeitige Renders (2), Java-Starts (2) und
  Passphrase-Ableitungen (1) sind begrenzt. (W2)

## [2026.5.1] — Hotfix, veröffentlicht 2026-05-28

### Fixed

- **AfA-Formular crashte sofort beim Öffnen in V2026.5.0** mit
  `Konfiguration: afa-tabellen.json nicht lesbar
  (…\AppData\Local\Klein.Buch\inputs\specs\afa-tabellen.json): Das System
  kann den angegebenen Pfad nicht finden`. Ursache: `inputs/` war in
  `tauri.conf.json` `bundle.resources` nicht aufgeführt, und
  `Paths::inputs_dir` zeigte im Release auf `resource_dir().join("inputs")`,
  also ins Leere. `afa_tabellen::load` hatte — anders als die PDF-Vorlagen —
  keinen eingebetteten Fallback. (Block **R7-INPUTS** + Folge-Fix
  `block-r7-inputs-fix` für den `resources/`-Pfad-Prefix beim Lesen des
  Bundle-Mirrors.)

  Dieselbe Klasse Bug betraf latent `inputs/mail-templates/` (Mail-Versand)
  und `inputs/branding/logo.png` (PDF-Logo); PDF-Rechnungsvorlagen
  überlebten über die eingebetteten `builtin_unified`-Fallbacks.

### Changed

- **`inputs/` ist jetzt vom Bundle entkoppelt:**
  `klein-buch/src-tauri/build.rs` spiegelt
  `klein-buch/inputs/{specs,pdf-templates,mail-templates,branding}` nach
  `src-tauri/resources/inputs/` (gitignored), das landet via
  `bundle.resources` im NSIS-Setup. `Paths::inputs_dir` zeigt in Production
  jetzt auf `app_local_data_dir/inputs/` — der einzige Pfad, in den der
  User aus eigener Kraft schreiben kann (BMF-AfA-Tabellen-Updates, eigene
  PDF-/Mail-Templates). Im Dev-Build (`cfg!(debug_assertions)`) bleibt der
  direkte Repo-Pfad.
- **Neuer First-Run-Copy** (`config::ensure_inputs_seeded`,
  `db::prepare_filesystem`): kopiert beim ersten Start das gebundelte
  `resource_dir/inputs/` rekursiv in `app_local_data_dir/inputs/`,
  **idempotent + ohne Überschreiben** (`inputs/`-Hardline aus
  `CLAUDE.md`: User-Edits an `afa-tabellen.json` oder eigene Vorlagen
  überleben jeden App-Start unverändert). Schema bleibt v30.

### Internal

- Tests `klein-buch/src-tauri/tests/inputs_seed_test.rs` decken
  missing-files, no-overwrite, mixed-gaps, idempotent, nested-subdirs und
  empty-bundle ab.
- `docs/RELEASE-1.0-GUIDE.md` (privat) erhält die Sektion
  „G4-Pre-Tag-Gate — Install-Smoke": vor jedem Phase-Tag muss das
  NSIS-Setup auf einem sauberen Windows-Profil installiert + die
  Hauptseiten (AfA-Formular, Vorlagen, Branding, Mail) einmal getouched
  werden. R1–R6 (v2026.5) hatten diesen Schritt nicht, deshalb ist
  R7-INPUTS bis ins v1.0-GA durchgerutscht.

## [2026.5.0] — v1.0 GA, veröffentlicht 2026-05-27

Erster öffentlicher Release auf
<https://github.com/hozwei/klein.buch-public/releases/tag/v2026.5.0>
(Block G4.5). Inhalt = Stand auf `main` nach G4.3 (Release-Pipeline-
Härtung). Privates Build-Repo `hozwei/klein-buch` bleibt privat
(Build-Journal, Memory, QA-Logs werden bewusst nicht freigegeben);
veröffentlicht wurde aus dem separaten Public-Repo
`hozwei/klein.buch-public` (gleicher Source-Stand, ohne interne
Notes). Tag `v2026.5.0` zuerst lokal auf dem privaten `main` gesetzt
(Block G4.4); die Veröffentlichung in G4.5 erfolgte per
`workflow_dispatch` im public Repo (`release.yml` mit Tag-Input
`v2026.5.0`), Draft mit eigenem Release-Notes-Body (Highlights +
SmartScreen-Hinweis + SHA-256-Anleitung + AGPL-§13-Source-Link) als
„latest release" publiziert. Artefakte:
`klein-buch_2026.5.0_x64-setup.exe` + `SHA256SUMS.txt`. Pre-Publish-
Security-Audit grün (keine Tokens, keine echten Bank-/Steuer-/
Bestandsdaten, keine DB-Snapshots; AGPL-§13-Pflichtdaten im
AboutDialog bewusst öffentlich).

### Changed

- **Release-Bundle nur noch NSIS, MSI verworfen** (ADR 0038, Block G4.3).
  Hintergrund: Windows-Installer `ProductVersion` darf im `MAJOR`-Feld
  maximal 255 sein, CalVer-Jahr `2026` sprengt das. Tauri 2 hat keinen
  offiziellen Override-Hebel; eigene WiX-Templates wären Bürokratie für
  ein Format, das im §19-Kleinunternehmer-Use-Case keinen Mehrwert hat
  (MSI ist primär Active-Directory-/GPO-Massendeployment). NSIS deckt den
  Single-User-Install-Pfad voll ab. `tauri.conf.json::bundle.targets` ist
  jetzt `["nsis"]` statt `"all"`; der Release-Workflow sammelt SHA-256-
  Summen nur über `.exe`.

### Internal (Release-Pipeline-Härtung beim Dry-Run G4.3)

- `.github/workflows/release.yml` `workflow_dispatch`-Default-Tag von
  `v0.1.0` auf `v2026.5.0-rc1` gehoben.
- `src-tauri/about.toml::targets` als String-Liste statt Inline-Table-
  Form geschrieben (cargo-about 0.6 lehnt die `{ triple = "…" }`-Form
  ab).
- `src-tauri/Cargo.toml` mit `publish = false` markiert, damit
  `cargo about generate` die Eigen-Crate aus der Drittanbieter-Lizenz-
  Liste herausfiltert (`private = { ignore = true }` greift). Die
  Eigen-Lizenz `AGPL-3.0-or-later` taucht so korrekt nicht als
  „akzeptierte Drittanbieter-Lizenz" auf.
- `CDLA-Permissive-2.0` (Mozilla CA Root Bundle via `webpki-roots`,
  transitiv über `russh`/`rustls`) in `accepted` aufgenommen.

## [Unreleased] — Post-v1.0 (Phase 7, E-Rechnungs-Politur)

### Added

- Rechnungs-Eingang (intern `drop_folder`) für eingehende E-Rechnungen
  (PV1-DROP, ADR 0037). Ein überwachter Ordner in den Einstellungen;
  abgelegte XRechnung- oder ZUGFeRD-Dateien werden im 5-Minuten-
  Scheduler-Tick (plus App-Start-Sweep und manueller „Jetzt prüfen"-
  Button) eingelesen und über dieselbe Pipeline wie der manuelle
  E-Rechnungs-Import verarbeitet. Erfolgreiche Übernahmen wandern nach
  `processed/YYYY-MM/`, fehlerhafte nach `failed/` (kein Auto-Delete,
  Diagnose-Pfad). Inbox-only-Notifications (`rule_drop_folder_import_ok`
  default off, `rule_drop_folder_import_failed` default on). Polling
  statt `notify`-Crate wegen OneDrive-Sync-Quirks. Migration
  `0029_drop_folder.sql`, Schema `v28 → v29`.
- Roh-XML-Viewer auf der Eingangsbeleg-Detailseite (PV1-A5, ADR 0037).
  Neuer Button „Roh-XML anzeigen", öffnet einen Modal-Dialog mit der
  ursprünglichen XML aus dem Archiv (bei ZUGFeRD via Mustang-Bridge aus
  dem PDF/A-3 extrahiert). Pretty-Print, Copy-Button, SHA-256-Hash und
  Byte-Größe. Liest still über `archive::store::read_and_verify_silent`,
  kein `archive.read`-Audit pro Klick. Tamper-Detection bleibt aktiv.
- Zwei neue Handbuch-Seiten: „Eingangsrechnungen via Ordner" und
  „Probleme mit dem Rechnungs-Eingang". Updates an Kosten erfassen,
  Einstellungen, Glossar und FAQ. ADR `0037-einvoice-receive-polish-
  drop-folder.md`. Tech-Doku (`ARCHITECTURE.md`, `architecture/{modules,
  adr-index,data-model,handbook}.md`) auf Schema v30 + ADRs 0001–0037
  aktualisiert.

### Changed

- E-Rechnungs-Empfang lehnt ZUGFeRD-Profile `MINIMUM` und `BASIC-WL`
  jetzt mit einem klaren Fehler ab (`ParseError::UnsupportedProfile`),
  statt sie indirekt über einen KoSIT-Fail durchzuwinken (PV1-A2,
  ADR 0037 D-73). Akzeptierte Profile: `EN16931`, `EXTENDED`,
  `XRECHNUNG` (Substring-Match, case-insensitive). Beide abgelehnten
  Profile sind seit der E-Rechnungs-Pflicht 2025 keine gültigen
  E-Rechnungen mehr.
- UI-Label „Drop-Folder" → „Rechnungs-Eingang" in der Settings-Card,
  PageBar, Toggle, Eingabe-Hinweisen, Toast-Meldungen, Notification-
  Titeln und den zwei Notification-Regel-Labels (PV1-RENAME, CLAUDE.md-
  UI-Sprachregel + Plain-Language-Hardline aus G2-DOC.2). Button
  „Jetzt synchronisieren" → „Jetzt prüfen". Code-Identifier
  (`drop_folder_*`, Route `/settings/drop-folder/`, Module-Namen,
  Rule-IDs, Migration-Filenames) bleiben englisch nach der CLAUDE.md-
  Hard-Rule „Code/Identifier/Commits englisch". Migration
  `0030_rename_drop_folder_labels.sql`, Schema `v29 → v30`.

## [Unreleased] — Phase 2D (wird `v0.1.0`)

Notifications + prüfungssicherer Geschäftsjahres-Abschluss, OAuth-Versand über
Microsoft Graph, durchgängiges Design-System und wählbare PDF-Vorlagen. Tag
`v0.1.0` folgt nach dem Cross-OS-Release-Workflow (Block 17d).

### block-17c — Doku + ADRs (2026-05-22)

- ADRs `0023`–`0030` ergänzt (Recurring/Scheduler, E-Rechnung-Empfang,
  Anlagen/AfA, EÜR-Export, Notifications/GJ-Lock, OAuth/E-Mail-Protokoll,
  Design-System, wählbare PDF-Vorlagen).
- README auf den vollständigen Feature-Stand gebracht; ARCHITECTURE +
  user-guide + operations-guide nachgezogen.

### block-17b — Konsolidierter Happy-Path-E2E-Test (2026-05-22)

- `tests/happy_path_test.rs`: Stammdaten (§19) → Kontakt → Rechnung → echte
  `run_lock_pipeline` (Mock-Sidecar, inkl. XRechnung + §19-Klausel-Check +
  Typst-Render + ZUGFeRD-Mock + Archiv) → Vollzahlung → Kosten → **EÜR**.
  Cross-Modul-Assertion: Rechnung = Betriebseinnahme, Kosten = Ausgabe,
  Überschuss = Differenz. Läuft ohne MailHog/Java in jedem `cargo test`.
- **Entscheidung:** Rust-Integrationstest statt Playwright (passt nicht zu
  Tauri; offizieller UI-E2E-Weg wäre tauri-driver/WebdriverIO). Siehe TASKS.md.

### block-17a — Wählbare PDF-Vorlagen + Switcher (2026-05-22)

- 3 eingebettete Unified-Templates (`modern`/`klassisch`/`minimal`,
  `pdf::templates`): eine `.typ` rendert Rechnung **oder** Angebot via internem
  Branching, jede mit `// §19-KLAUSEL-BLOCK: REQUIRED` + Klauseltext + Logo.
- Doctype-bewusster Resolver (`resolve_invoice_template`/`resolve_quote_template`)
  mit `inputs/`-Override-Vorrang und `default`-Kollisions-Schutz; `list_templates`
  merged Built-ins + eigene Dateien (`TemplateMeta.builtin`).
- Globale Auswahl via `seller_profile.default_pdf_template` (Sentinel: Beleg-
  `'default'` → globaler Default beim Render; archiviertes PDF = GoBD-Snapshot).
- Switcher `settings/pdf-templates` mit §19-/Built-in-Badges, **Vorschau-PDF**
  je Vorlage (Dummy „Max Mustermann" + geometrisches Dummy-Logo). §19-Schutz:
  nicht-konforme Vorlagen bei Kleinunternehmer gesperrt. Logo-Rendering via
  Typst `World::file()`. Siehe ADR 0030.

### block-ds — Design-System + UI-Konsistenz (2026-05-22)

- Zentrale Design-Tokens (`src/lib/styles/tokens.css`: Petrol `#176b87` +
  SaaS-Look, System-Font-Stack/local-first) + Komponenten `Button`/`Card`/
  `Badge`/`FormField`/`Table`/`PageBar`/`ToastHost`/`ConfirmDialog`/`Banner`.
- Feedback vereinheitlicht: Toast als Standard, `confirmDialog()` statt
  Browser-`confirm()`, Banner für persistente Zustände, Modal für
  folgenreiche Aktionen. Sticky `PageBar` (Zurück links, Aktionen rechts) auf
  allen Seiten; Firmenlogo-Upload. Siehe ADR 0029 (+ ADR 0021).

### block-16b — E-Mail-Versandprotokoll + Suche (2026-05-22)

- Migration `0015_email_log` (append-only, `EXPECTED_SCHEMA_VERSION = 15`):
  protokolliert jeden Versand (Erfolg + Fehler) mit Provider-Antwort (SMTP-Code
  / Graph-`request-id`) + serverseitiger Suche/Filter. Seite
  `settings/mail-log` + Versand-Historie auf Rechnung/Angebot.

### block-16 — OAuth Microsoft Exchange Online (2026-05-22)

- Migration `0014_oauth` (5 nicht-geheime OAuth-Spalten, Schema v14). Versand
  über **Microsoft Graph `/me/sendMail`**, nutzer-eigene Azure-App (Loopback +
  Public-Client + PKCE). **Nur der Refresh-Token** im OS-Keychain, **gechunkt**
  (Windows-2560-Limit). `settings/mail` mit Verbinden/Trennen + Azure-Hilfe.
  Siehe ADR 0028.

### block-15 — Notifications + GJ-Lock + Integritäts-Cron (2026-05-22)

- Migrationen `0012_notifications` + `0013_fiscal_year_locks` (Schema v13).
- In-App-Inbox (Quelle der Wahrheit) + OS-Notifications mit Dedup; 4
  Reminder-Regeln. Prüfungssicherer GJ-Abschluss (§146 AO): AfA buchen → Anlagen
  + Abschreibung sperren → EÜR-Snapshot → Audit → Auto-Backup; nur abgelaufene
  Jahre, Backup-Unlock-Pflicht, unumkehrbar; `guard::ensure_year_open` in den
  Command-Wrappern (Storno bleibt möglich). Auto-AfA am 01.01. (Default an,
  abschaltbar). Monatlicher Archiv-Integritäts-Check. Audit-Trail-Ansicht.
  Siehe ADR 0027.

## [0.1.0-phase2c] - 2026-05-21

Phase 2C — Anlagenverzeichnis + AfA + EÜR + Steuerberater-Export (Blöcke 12–14).

### block-14 — EÜR-Export + Steuerberater-Paket (2026-05-21)

- Keine Migration (Schema v11). ELSTER-Ausfüllhilfe (Anlage-EÜR-Zeilen-Mapping)
  + Typst-PDF „Anlage EÜR" (Anlage EÜR + AVEÜR + Einzelaufstellung). DATEV-
  Buchungsstapel (EXTF, **SKR03-Default**/SKR04, CP1252/CRLF, voll inkl. AfA).
  Steuerberater-Paket als ZIP (Deckblatt + EÜR-PDF + DATEV + Einzel-CSVs +
  Stammdaten). AfA-Safeguard-Banner. Reference-Docs ELSTER + DATEV. Siehe
  ADR 0026. Tag `v0.1.0-phase2c`.

### block-13 — EÜR-Aggregation (Cash-Basis) (2026-05-21)

- Keine Migration (Schema v11). `euer::aggregate` (Functional Core) + Repo +
  Commands + `routes/euer`. Cash-Basis nach §11 EStG / §4 Abs. 3 EStG:
  Einnahmen am Zahlungseingang (bleiben im Zuflussjahr), Storno = negative
  Einnahme zum Storno-Datum, Kosten am Zahlungsausgang, AfA als Jahresgröße,
  Anlagen-Veräußerung, Privatbewegungen EÜR-neutral. Siehe ADR 0022 (korrigiert
  ADR 0010).

### block-12 — Anlagenverzeichnis + AfA (2026-05-21)

- Migrationen `0010_assets` + `0011_depreciation` (Schema v11). AfA-Berechnung
  als Functional Core (`domain::depreciation`), AfA-Tabellen als JSON in
  `inputs/`; linear + GWG-Sofortabschreibung + Computer-Sonderregel +
  Privatanteil + Veräußerung. Idempotenter AfA-Lauf (`UNIQUE(asset_id,
  fiscal_year)`); Festschreibung erst zum GJ-Abschluss. Siehe ADR 0025.

## [0.1.0-phase2b] - 2026-05-21

Phase 2B — Kosten, Wiederkehrendes, E-Rechnung-Empfang (Blöcke 9–11).

### block-11 — E-Rechnung-Empfang (2026-05-21)

- Keine Migration (Schema v9). `einvoice::parser` liest XRechnung (CII + UBL)
  und extrahiert CII aus ZUGFeRD-PDFs. KoSIT-Validierung beim Empfang
  **beratend** (nie blockierend); Import → normale Kostenposition; Original
  write-once archiviert (`ReceivedEinvoice`). `routes/expenses/import`. Siehe
  ADR 0024. Tag `v0.1.0-phase2b`.

### block-10 — Recurring + Scheduler-Foundation (2026-05-20)

- Migration `0009_recurring`. `scheduler::{tick, recurring}`: 5-Minuten-Tick
  nach Bootstrap, Unlock-gated; Auto-Anlage fälliger Belege mit `paid_date NULL`
  + Catch-up für Ausfallzeiten. Siehe ADR 0023.

### block-9 — Kosten + Anhänge + §13b + Privatbewegungen + Payment-Accounts (2026-05-20)

- Migrationen `0007_expenses` + `0008_private_movements`. Ausgaben mit
  Beleg-Upload, §13b-Reverse-Charge, sofort gelockt; §19 wirkt nur auf der
  Ausgangsseite → Kosten gehen **brutto** in die EÜR. Privatentnahmen/-einlagen
  EÜR-neutral. Zahlungskonten (`payment_accounts`). Siehe ADR 0019 + ADR 0020.

## [0.1.0-phase2a] - 2026-05-20

Phase 2A — Angebote (Blöcke 6–8): Angebote anlegen, festschreiben, annehmen
(mit Vertrags-Upload), in Rechnungen konvertieren, als PDF erzeugen und als
**Bundle** (Angebot + AGB + Datenschutz) versenden. Zentral versionierte
Rechtsdokumente mit fester, unveränderlicher Verknüpfung pro Angebot.

### block-8 — Angebote-PDF + Versand + Bundle + Rechtsdokumente (2026-05-20)

- Migration `0006_legal_documents.sql`, `EXPECTED_SCHEMA_VERSION = 6`:
  - `legal_documents` (versionierte AGB/Datenschutz-PDFs: `doc_type`, `version`
    monoton pro Typ, `title`, `archive_entry_id`, `is_active`). Partial-unique
    `uq_legal_documents_active` = höchstens eine aktive Version pro Typ. Trigger
    `no_delete` (append-only) + `immutable` (nur Aktiv-Status änderbar).
  - `quote_legal_documents` (append-only Bindung Angebot↔ausgegebene Version,
    `version`-Snapshot, unique pro (Angebot, doc_type), no-delete + immutable).
- `ArchiveKind::LegalDocument`; `db::repo::legal_documents`
  (create_version/activate/deactivate/get/get_active/list +
  `bind_active_for_quote` idempotent + `list_for_quote`);
  `db::repo::quotes::set_pdf_archive_id`.
- `pdf::typst_render::render_quote` + `build_quote_data_json`; Refactor
  `compile_pdf` (Rechnung = PDF/A-3b für Mustang, **Angebot = Plain-PDF**, keine
  E-Rechnung). `pdf::templates::{DEFAULT_QUOTE_TEMPLATE, load_quote_source}` —
  Quote-Template **eingebettet** (mit §19-Marker, durchläuft `klausel_check`),
  Override via `inputs/pdf-templates/quote.typ`, weil `inputs/` für Maschinen
  tabu ist.
- `pdf::bundle::merge_pdfs` — **neue Dependency `lopdf 0.40`**; führt Angebots-PDF
  + AGB + Datenschutz zu einem Druck-PDF zusammen (Merge-Rezept gegen das
  offizielle `lopdf/examples/merge.rs` verifiziert). Bundle wird nicht
  archiviert (abgeleitet); kanonisch sind Einzel-PDFs + Bindung.
- `mail::templates`: Angebots-Mail (`QuoteMailContext`, eingebettetes
  `DEFAULT_QUOTE_MAIL`, `load_quote_template`, `render_quote_mail`).
  `commands::mail::{mail_quote_preview, mail_send_quote, send_quote_core}` —
  Bundle als **Multi-Attachment** (3 Dateien), Audit `quote.sent` mit
  gebundenen Versionen, kein Status-Wechsel (Angebot ist ab Festschreiben `sent`).
- `commands::legal_documents::*` (Upload/List/Activate/Deactivate),
  `commands::quotes::{ensure_quote_pdf, bind_legal_docs_for_quote,
  prepare_quote_dispatch, quotes_generate_pdf, quotes_open_bundle,
  quotes_legal_bindings}`.
- Frontend: `settings/legal` (Upload + Versionen + Aktivieren), Angebots-Detail
  (PDF anzeigen / Bundle drucken / Versenden + gebundene Rechtsdokumente),
  `quotes/[id]/send` (mit Legal-Docs-Pflichtprüfung), `api.ts`/`types.ts`.
- **Design-Entscheidungen:** Legal-Docs als PDF-Upload pro Version (kein
  In-App-Editor); Druck = ein zusammengeführtes PDF, Mail = 3 Anhänge; Bindung
  bei Bundle-/Versand-Erzeugung (idempotent, append-only) + **Pflicht für
  Versand** (aktive AGB + Datenschutz). Siehe ADR 0018.
- Tests: `tests/legal_documents_repo_test.rs` (Versionierung, single-active,
  Immutability/No-Delete-Trigger, idempotente Bindung) + Unit-Tests
  `pdf::bundle`, `pdf::templates`, `pdf::typst_render` (render_quote),
  `mail::templates` (Quote-Mail).
- Juristik-Caveat: AGB-/Datenschutz-Texte vor Echtbetrieb anwaltlich prüfen.

### block-7 — Angebot → Rechnung-Konvertierung (2026-05-20)

- **Keine Migration** (nutzt `derived_from_quote_id` + `converted_*` aus
  `0005_quotes`; Schema bleibt v5 bis Block 8). `domain::quote::convert_to_invoice`
  (pure, 1:1-Item-Mapping inkl. §19-Carry-over). `db::repo::quotes::mark_converted`
  (Guard `accepted → converted`). Gemeinsamer Draft-Helper
  `commands::invoices::create_invoice_draft_from_input` (von Neuanlage UND
  Konvertierung genutzt, setzt `derived_from_quote_id`).
- **Hard-Rule:** Konvertierung nur aus `status='accepted'`. Die Konvertierung
  erzeugt eine Rechnungs-**Draft**, die über die normale Lock-Pipeline
  festgeschrieben wird (kein Issue-Duplikat). Siehe ADR 0017.
- Frontend `routes/quotes/[id]/convert` (Positionen vorbefüllt + anpassbar).

### block-6 — Angebote: Schema + CRUD + Annahme-Workflow (2026-05-20)

- Migration `0005_quotes.sql` (`quotes` + `quote_items` + `trg_quotes_immutable`
  + `invoices.derived_from_quote_id`), `EXPECTED_SCHEMA_VERSION = 5`;
  `seller_tax_number` nullbar (konsistent mit Rechnung/Seller, §33-Compat).
- Eigener Belegkreis `AN-{YYYY}-{NNNN}`. Lifecycle
  `draft → sent → accepted|rejected → converted | canceled`. `domain::quote`
  (Functional Core: `compute_totals` (reuse Invoice-Totals), `validate_quote` mit
  §19-`assert_no_vat`). `db::repo::quotes` (CRUD + State-Transitions mit
  Status-Guards) + `db::repo::attachments` (generische Eltern-Verknüpfung
  archivierter Dateien).
- **Festschreiben = Lock → `sent`** (kein eigener `issued`-Status für Angebote);
  ab Lock greift `trg_quotes_immutable`. Annahme archiviert optional den
  **unterschriebenen Vertrag** write-once als Attachment. Siehe ADR 0016.
- Frontend `routes/quotes/*`; `attachments_open`/`_reveal`-Commands.

## [0.1.0-phase1] - 2026-05-20

Erster produktiv nutzbarer Stand (Walking Skeleton, Blöcke 0–5): Kontakte,
Rechnungen mit ZUGFeRD-PDF/A-3, SMTP-Versand, Storno, GoBD-Archiv, verschlüsseltes
Backup/Restore, Migrations-Export.

### block-5 — SMTP-Versand + Phase-1-Polish (2026-05-20)

- **Keine Migration** (`mail_accounts` bereits aus `0001_init.sql`);
  `EXPECTED_SCHEMA_VERSION` bleibt 4.
- `mail::keyring`: SMTP-Postfach-**Passwort** im OS-Keychain, Service-ID-Schema
  `kleinbuch::mail::{account_id}`. **`keyring`-Crate-Features** ergänzt
  (`apple-native`/`windows-native`/`sync-secret-service`/`crypto-rust`) — ohne sie
  fiele keyring 3 auf einen flüchtigen Mock-Store zurück (Passwort nach Neustart
  weg). Passwort nie in DB/Logs/Audit. (Nicht zu verwechseln mit der
  Backup-Passphrase — die lebt nur im Session-Memory, nicht im Keychain.)
- `mail::smtp`: lettre (async, rustls), **Multi-Attachment von Anfang an**
  (Grundlage Angebots-Bundle Block 8), TLS/STARTTLS/Klartext je nach Konfiguration,
  `test_connection`. Anhanglose Mails werden als reine `text/plain` gebaut (kein
  `multipart/mixed`-mit-einem-Teil); mit Anhang `multipart/mixed`. Message-Building
  ist pure und unit-getestet. `tracing`-Logs
  für Verbindung + Versand (Host/Port/TLS/User/Empfänger/Ergebnis) — **nie das
  Passwort**; lettres rohes `tracing`-Feature bewusst aus (würde AUTH mitloggen).
- `mail::templates`: Tera-Render von `inputs/mail-templates/invoice-de.txt`
  (Subject/Body-Split, deutsche Betrags-/Datumsformatierung).
- `db::repo::mail_accounts` (CRUD-Subset + `touch_last_used`) +
  `invoices::mark_sent` (`status='issued'→'sent'`, `sent_at`; GoBD-konform, da nicht
  in der Immutability-Whitelist).
- `commands::mail`: `mail_accounts_list`, `mail_account_create`/`_update`/`_delete`
  (Passwort → Keychain; Delete entfernt auch den Keychain-Eintrag),
  `mail_account_test_connection`, `mail_send_test` (Test-Mail über ein
  gespeichertes Konto), `mail_invoice_preview`, `mail_send_invoice`,
  `mail_send_quote` (Guard bis Block 8). `send_invoice_core` liest das ZUGFeRD-PDF
  aus dem Archiv **mit SHA-256-Verify**, rendert den Body, versendet, setzt `sent`,
  schreibt Audit `invoice.sent`. Neue `Error::Mail`-Variante.
- Frontend: `routes/settings/mail` (Konten-Liste mit **Bearbeiten/Löschen** +
  Formular + Test-Connection + **Test-Mail senden** an beliebigen Empfänger),
  `routes/invoices/[id]/send`
  (Konto-Auswahl, Empfänger-Default aus Buyer-Snapshot/Kontakt, Body-Vorschau,
  ZUGFeRD-Auto-Anhang), „Senden"-Button auf der Rechnungs-Detailseite.
- E2E: `tests/e2e_test.rs` (Seller → Kontakt → Rechnung → Versand gegen MailHog,
  Assertions über die MailHog-API + DB-Status + Audit). MailHog als CI-Service in
  `ci.yml`; lokal ohne MailHog überspringt sich der Test (no-op).
- Doku: ARCHITECTURE.md ausgebaut, ADRs `0002`–`0015` (Decision-Log §9),
  `docs/user-guide.md` (§1–3), `docs/operations-guide.md` (Stub).

### block-4 — Backup + Restore + Migrations-Export (2026-05-20)

- Keine Migration nötig (`backup_history` bereits aus `0001_init.sql`);
  `EXPECTED_SCHEMA_VERSION` bleibt 4. Backup-Ziel + Passphrase-Verifier liegen
  als `app_settings`-Keys (kein Schema-Impact).
- `backup::manifest` + `backup::encrypt`: Argon2id (m=64 MB, t=3, p=4) +
  AES-256-GCM, frische Salt/Nonce pro Backup, eigener Hex-Codec (kein neues
  Cargo-Dep). Datei-Format: Plain-Header (MAGIC + Manifest-JSON) + verschlüsselter
  Body.
- `backup::snapshot`: konsistenter DB-Snapshot via `PRAGMA wal_checkpoint(TRUNCATE)`
  + Datei-Lesen (statt `VACUUM INTO`); ZIP aus DB + `archive/` + `inputs/branding/`.
- `backup::target` (konfigurierbares Ziel, OneDrive-tauglich) + `backup::rotation`
  (GVS: 30 daily / 12 monthly / 7 yearly; `manual`/`pre_restore` nie geprunt).
- Passphrase lebt nur im Session-Memory (`BackupSession`, Tauri-State) — nie in
  DB/Logs/Audit. Setup/Unlock über verschlüsselten Verifier (kein Klartext/Hash).
- `backup::create_now`-Pipeline (Snapshot → Encrypt → Manifest → Ziel →
  `backup_history` → Rotation). Auto-Critical-Backup nach Invoice-Lock und Storno
  (best-effort), Auto-Daily nach Unlock (letztes Backup > 24 h).
- Restore zweiphasig: Phase A (Pre-Restore-Backup-Pflicht + Decrypt + Hash-Verify
  + Staging + Marker), Phase B (DB-/Archive-Swap beim App-Start vor Pool-Open
  wegen Windows-File-Lock). `inputs/` bleibt beim Restore tabu (nur DB + Archive).
- `migration_export`: offenes ZIP (JSON pro Tabelle, `archive/`, Schema-SQL aus
  MIGRATOR, `erd.md`, `manifest.json`, `read_export.py`-Standalone-Reader).
- Tauri-Commands `backup_*` + `migration_export_run`, in `lib.rs` registriert.
- Frontend: `BackupGate` (Onboarding erzwingt Passphrase, Unlock beim Start),
  `routes/settings/backup` (Status/Ziel/manuell/Verlauf/Restore-Wizard mit
  GoBD-Warndialog), `routes/settings/migration-export`.
- Tests: Unit-Tests je Modul + `tests/backup_restore_test.rs` (Backup → Decrypt →
  Restore, Tamper-Detection, Export-Roundtrip).

### block-3 — Rechnung erstellen: Issue + Storno (3a–3d, 2026-05-19/20)

- Migrationen `0003_invoices_seller_tax_number_optional.sql` (§33-Kleinbetrags-
  rechnung) und `0004_invoices_buyer_snapshot.sql` (Empfänger-Snapshot auf
  Rechnungen für spätere DSGVO-Anonymisierung). `EXPECTED_SCHEMA_VERSION = 4`.
- `domain::{invoice,kleinunternehmer,kleinbetragsrechnung,storno,numbering}`:
  Functional Core mit `validate_for_issue` (§14 + §19 + §33), Cent-basierte
  kaufmännische Rundung, `assert_no_vat` §14c-Schutz.
- `einvoice::generator`: XRechnung als **UN/CEFACT CII** (ZUGFeRD/Mustang bettet
  ausschließlich CII ein), §19-Klausel als BT-22-Note + BT-120 ExemptionReason,
  Storno mit Type-Code 384 + BillingReference.
- `einvoice::validator` (KoSIT-Bridge) + `einvoice::mustang_bridge`
  (ZUGFeRD-PDF/A-3), beide mit Mock-Mode für Tests.
- `pdf::typst_render` (PDF/A-3b) + `pdf::klausel_check` (strenger Pre-Render-
  §19-Marker-Check).
- `archive::{store,integrity_check,audit}`: write-once, SHA-256, Tamper-Detection,
  read-only. `db::numbering`: lückenlose, parallel-sichere Nummernvergabe.
- `commands::invoices` mit `run_lock_pipeline` (validate → CII → KoSIT → klausel →
  typst → mustang → archive PDF+XML → DB-Lock → audit → backup-marker).
- Frontend: `routes/invoices/` (Liste/Detail/New/Cancel/Payment) mit
  §19-Hardline-Sperre, Live-Totals, Cash-Basis-Zahlungserfassung.
- Real auf Windows verifiziert: `RE-2026-0001.pdf` als ZUGFeRD-PDF/A-3 mit
  eingebettetem CII, KoSIT nicht-Failed, `verapdf` grün.

### block-2 — Kontakte + Seller Profile (2026-05-19)

- Migration `0002_seller_tax_number_optional.sql`: `seller_profile.tax_number`
  ist jetzt NULL-bar (Tabellen-Rebuild, weil SQLite NOT NULL nicht via ALTER
  entfernt). `EXPECTED_SCHEMA_VERSION = 2`. Onboarding ohne erteilte
  Steuernummer ist damit möglich; §14-UStG-Check wandert zu Block 3.

- `domain::contact`: Validierung (Name + Adresse Pflicht, deutsche PLZ-Form,
  DE-/EU-USt-IdNr.-Format, pragmatischer E-Mail-Check, IBAN-Längen-Check) als
  pure Funktion mit `ContactInput`/`ContactType` + `ValidationError`-Enum.
- `domain::kleinunternehmer`: `HINWEIS_TEXT` als wortgleicher §19-Pflichthinweis,
  `is_active()`, `must_show_hinweis()`, `waiver_deadline()` (01.01. des 6.
  Folgejahres nach §19 Abs. 2 UStG). `assert_no_vat()` bleibt für Block 3.
- `db::models`: `ContactRow` + `SellerProfileRow` als sqlx-`FromRow`,
  serde-camelCase für Tauri-Bridge.
- `db::repo::contacts`: CRUD + Suche + Archive/Unarchive. UUIDv7 als PK,
  kein DELETE.
- `db::repo::seller_profile`: Singleton get/upsert. §19→Regelbesteuerung-
  Wechsel setzt `waived_paragraph_19_since = today` + Audit-Log; verlangt
  explizite Bestätigung über `confirm_waive_paragraph_19`. Rückkehr zu §19
  innerhalb der 5-Jahres-Bindung wird im Backend abgelehnt.
- `db::repo::audit_log`: append-only Helper (`append`, `recent`).
- Tauri-Commands: `contacts_{list,get,create,update,archive,unarchive,search}`,
  `seller_profile_{get,upsert}`, `paragraph_19_info` — registriert in
  `lib.rs`.
- Frontend: TS-Types + typed API-Wrapper. Routes `/contacts` (Liste + Suche +
  Archive-Toggle), `/contacts/new`, `/contacts/[id]`, `/settings/seller`
  mit §19-Toggle + Modal-Dialog "Ich verstehe die 5-Jahres-Bindung".
- Integration-Tests: `tests/contacts_repo_test.rs` (CRUD + Suche + Archive
  + Validierung), `tests/seller_profile_test.rs` (Singleton + §19-Toggle +
  Audit-Log + 5-Jahres-Bindung).

### block-1 — Foundation + Phase-1-Schema (2026-05-19)

- Tauri-2 + Svelte-5 (TypeScript) + pnpm-Scaffold.
- Phase-1-DB-Schema: `app_settings`, `contacts`, `seller_profile`,
  `mail_accounts`, `invoices`, `invoice_items`, `archive_entries`,
  `attachments`, `audit_log`, `doc_number_counters`, `backup_history`,
  `archive_integrity_checks` — alle `STRICT`, Indizes auf allen Datums-
  und Foreign-Key-Spalten.
- GoBD-Triggers: `audit_log` append-only, `invoices` mit Lock-Immutability
  auf Kernfeldern, `archive_entries` mit Hash/Path/Size-Immutability.
- Schema-Version-Check beim App-Start (`EXPECTED_SCHEMA_VERSION = 1`),
  App startet nicht bei Mismatch.
- Module-Stubs für alle Phase-1- und Phase-2-Backend-Module
  (commands, db, domain, einvoice, pdf, archive, mail, backup,
  migration_export, scheduler, notify, euer, assets, depreciation,
  fiscal_year).
- Frontend-Routes: Top-Level pro Bereich + alle Settings-Sub-Routes.
- Java-Sidecar-Bundle (KoSIT 1.6.2 + XRechnung-Konfig 2026-01-31 +
  Mustang 2.23.0) im jlink-Bundle unter
  `src-tauri/binaries/klein-buch-java-x86_64-pc-windows-msvc/`,
  als `bundle.resources` registriert.
- AGPL-3.0-Header in `LICENSE`, README + CHANGELOG.
- GitHub-Actions-CI: cargo fmt + clippy + test + pnpm lint + tsc.
- Memory-Files in `~/cowork/Buchhaltung/memory/klein-buch/`.
- `inputs/`-Initial-Files: Spec-Stubs, `afa-tabellen.json` (BMF),
  `default.typ` mit §19-Klausel-Block-Marker, `invoice-de.txt` Tera-Template.

### Bekannte Einschränkungen

- Cross-OS-Bundles (macOS x86_64/aarch64, Linux x86_64) noch nicht gebaut.
  Folgen in Block 17 via GitHub-Actions-Matrix.
- Detail-Sub-Routes (`[id]/`, `new/`, `cancel/`, `send/`, `payment/`,
  `convert/`, `dispose/`, `import/`) werden in den jeweiligen Blöcken
  angelegt, in denen sie funktional benötigt werden — nicht alle in Block 1.

## Geplant

- Phase 2B (Blöcke 9–11): Kosten + Anhänge + §13b + Privatbewegungen +
  Payment-Accounts, Recurring/Scheduler, E-Rechnung-Empfang → **Tag `v0.1.0-phase2b`**.
- Phase 2C (12–14): Anlagenverzeichnis + AfA + EÜR + Steuerberater-Export.
- Phase 2D (15–17): Notifications, OAuth (Exchange Online), PDF-Templates + E2E +
  Release `v0.1.0`.
- Phase 3 (18–19): DSGVO-Auskunft + -Anonymisierung.
