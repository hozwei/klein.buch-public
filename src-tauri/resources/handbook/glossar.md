---
slug: glossar
title: Glossar
category: glossar
order: 10
keywords: [glossar, fachbegriffe, definitionen, abkürzungen, kleinunternehmer, §19, gobd, eür, afa, xrechnung, zugferd, passphrase, sidecar, rechnungs-eingang, drop-folder, roh-xml, zugferd-profil]
---

# Glossar

Dieses Kapitel erklärt alle Fachwörter und Abkürzungen, die in
Klein.Buch und in den anderen Handbuch-Kapiteln auftauchen. Wenn Du
irgendwo im Text ein Wort siehst, das Du nicht kennst, schau hier
nach. Die Einträge stehen alphabetisch. Die Paragraphen aus den
Steuergesetzen findest Du am Ende als eigenen Block.

Das Glossar ist die verbindliche Quelle für jede Fachbegriff-
Definition in diesem Handbuch. Wenn ein Kapitel einen Begriff
verwendet, gilt die Erklärung von hier.

## Abo-Rechnung

Eine Rechnung, die in regelmäßigen Abständen automatisch wieder
erzeugt wird (zum Beispiel jeden Monat oder jedes Quartal). Du legst
eine Vorlage an und sagst Klein.Buch, in welchem Rhythmus daraus
neue Rechnungen werden sollen. Wird auch "wiederkehrende Rechnung"
genannt. Klein.Buch kann diese Rechnungen auf Wunsch direkt
abschicken, sobald sie fällig sind.

## AfA (Absetzung für Abnutzung)

Der jährliche Wertverlust teurer Anschaffungen, den Du über mehrere
Jahre verteilt steuerlich absetzt. Beispiel: Du kaufst einen Laptop
für 1.500 €. Statt die 1.500 € im Kaufjahr komplett abzuziehen,
verteilst Du sie über die Nutzungsdauer laut amtlicher Tabelle
(beim Laptop sind das 3 Jahre, also 500 € pro Jahr). Klein.Buch
führt die AfA-Tabelle automatisch und übernimmt die Beträge in
Deine EÜR.

## Anfahrt

Eine Beleg-Position, die Du nach gefahrenen Kilometern und einem
Kilometer-Satz abrechnest, wenn Du zum Kunden fährst. Klein.Buch
rechnet das Produkt automatisch aus, sobald Du Kilometer und Satz
einträgst.

## Anlage

Ein teurer Gegenstand, den Du nicht im Kaufjahr komplett absetzt,
sondern über mehrere Jahre verteilt (siehe AfA). Typische Anlagen
sind Notebook, Werkstattmaschine, Geschäftswagen.

## Anonymisierung (DSGVO)

Das Löschen aller persönlichen Daten eines Kontakts, entweder auf
Deine eigene Entscheidung oder auf Anforderung der betroffenen
Person. Klein.Buch ersetzt Name, Adresse und Notizen durch leere
Felder. Die alten Rechnungen bleiben als Buchhaltungs-Beleg
unverändert erhalten (das ist gesetzliche Pflicht laut GoBD, kein
Bug). Siehe auch DSGVO.

## Aufbewahrungsfrist

Die gesetzliche Pflicht, bestimmte Belege eine Mindestzeit
aufzuheben. Für Rechnungen und alle steuerlich relevanten Belege
sind das in Deutschland 10 Jahre nach Ende des jeweiligen
Geschäftsjahres. Klein.Buch hat absichtlich keine Lösch-Funktion
für festgeschriebene Belege, damit Du diese Pflicht nicht
versehentlich brichst. Gesetzliche Grundlage: §147 AO.

## Auskunft (DSGVO)

Das Recht jeder Person, von Dir zu erfahren, welche Daten Du über
sie gespeichert hast. Klein.Buch erzeugt Dir die Auskunft mit
einem Klick: ein ZIP-Paket mit einer PDF-Zusammenfassung, allen
Rechnungen zu dieser Person und den Daten in maschinenlesbarem
Format. Gesetzliche Grundlage: DSGVO Art. 15.

## Backup

Eine verschlüsselte Sicherheits-Kopie Deiner Datenbank und Deines
Beleg-Archivs. Klein.Buch schreibt automatisch ein Backup nach jeder
Festschreibung und einmal pro Tag. Jedes Backup ist mit Deiner
Passphrase verschlüsselt; ohne Passphrase ist es wertlos.

## Backup-Ziel

Der Ordner oder Server, in den Klein.Buch die Backups schreibt. Du
kannst einen lokalen Ordner wählen (auf der Festplatte, einem
USB-Stick, einem Netzlaufwerk), einen Cloud-Ordner (OneDrive,
iCloud, Dropbox, Nextcloud, sofern auf Deinem Rechner als Ordner
sichtbar) oder einen SFTP-Server (siehe SFTP). Empfehlung: ein
lokaler Ordner als schneller Schutz plus ein Off-Site-Ziel gegen
Hardware-Schaden.

## Belegdatum

Das Datum, das oben auf der Rechnung steht. In der Regel der Tag,
an dem Du die Rechnung schreibst und festschreibst. Nicht zu
verwechseln mit dem Leistungsdatum (siehe dort).

## Brutto und Netto

Netto ist der Preis ohne Umsatzsteuer. Brutto ist der Preis
einschließlich Umsatzsteuer. Als Kleinunternehmer nach §19 UStG
weist Du keine Umsatzsteuer aus; für Deine Ausgangsrechnungen sind
Netto und Brutto deshalb identisch. Auf Eingangsrechnungen (Deinen
Kosten) steht aber oft Brutto mit ausgewiesener Umsatzsteuer, weil
der Verkäufer in der Regelbesteuerung sitzt. In der EÜR zählt bei
Kosten immer der Brutto-Betrag, weil Du keine Vorsteuer abziehen
darfst.

## Cash-Basis

Die einfache Buchhaltungs-Methode, die Klein.Buch nutzt. Eine
Einnahme zählt erst, wenn das Geld bei Dir eingegangen ist. Eine
Ausgabe zählt erst, wenn Du sie bezahlt hast. Das Gegenteil heißt
Soll-Versteuerung und ist für Kleinunternehmer nicht relevant.
Gesetzliche Grundlage: §11 EStG (Zufluss- und Abfluss-Prinzip).

## DATEV-Export

Ein Daten-Format, das praktisch jeder Steuerberater versteht.
Klein.Buch erzeugt Dir auf Knopfdruck einen DATEV-Stapel mit allen
Buchungen des Geschäftsjahres, den Dein Steuerberater direkt in
sein System importieren kann. Standard-Kontenrahmen ist SKR03.

## DSGVO (Datenschutz-Grundverordnung)

Das EU-Datenschutz-Gesetz. Es gibt jeder Person das Recht zu
erfahren, welche Daten Du über sie speicherst (Auskunft, Art. 15),
und unter bestimmten Bedingungen das Recht, dass Du diese Daten
löschst (Anonymisierung, Art. 17). Klein.Buch unterstützt beide
Funktionen über das Menü "Kontakte".

## EÜR (Einnahmen-Überschuss-Rechnung)

Die einfache Steuer-Aufstellung für Kleinunternehmer und kleine
Selbständige: Einnahmen minus Ausgaben. Du brauchst sie einmal im
Jahr für Deine Steuererklärung. Klein.Buch erstellt sie auf
Knopfdruck. Gesetzliche Grundlage: §4 Abs. 3 EStG.

## ELSTER

Das offizielle Online-Portal des Finanzamts. Dort gibst Du Deine
Steuererklärung digital ab. Klein.Buch exportiert die EÜR in einem
Format, das Du direkt in das ELSTER-Formular übertragen kannst
(diese Funktion heißt in der App "Ausfüllhilfe").

## Factory Reset

Das vollständige Zurücksetzen von Klein.Buch auf Werkszustand:
Datenbank, Beleg-Archiv, lokale Backups und Einstellungen werden
gelöscht. Off-Site-Backups (Cloud, SFTP) bleiben bestehen und musst
Du gegebenenfalls separat löschen. Vor dem Reset musst Du Deine
Passphrase eingeben und einen Hinweis auf die 10-Jahres-
Aufbewahrungspflicht ausdrücklich bestätigen.

## Festschreibung

Das endgültige Abschließen eines Belegs. Ab diesem Moment kannst
Du die zentralen Felder (Beträge, Kunde, Datum) nicht mehr ändern.
Eine festgeschriebene Rechnung lässt sich nur noch durch einen
Storno neutralisieren. Diese Sperre kommt aus der GoBD und schützt
Dich vor Vorwürfen bei einer Steuerprüfung.

## Geschäftsjahr

In Klein.Buch immer das Kalenderjahr (1. Januar bis 31. Dezember).
Jede Rechnung, Ausgabe und AfA-Buchung gehört zu genau einem
Geschäftsjahr. Die Rechnungs-Nummern starten in jedem neuen
Geschäftsjahr von vorn (zum Beispiel "2026-0001").

## Geschäftsjahr-Abschluss

Das endgültige Festschreiben eines kompletten Geschäftsjahres.
Danach kannst Du in dem abgeschlossenen Jahr nichts mehr ändern,
auch keine alten Belege nachtragen. Vorher solltest Du den
Steuerberater-Export erzeugt und die EÜR übermittelt haben.

## GoBD (Grundsätze ordnungsmäßiger Buchführung)

Die Regeln, nach denen das Finanzamt Buchhaltungs-Software
akzeptiert. Die wichtigsten Punkte: Belege sind nach der
Festschreibung unveränderlich, Storno ersetzt Löschung, alles wird
10 Jahre aufbewahrt, jede Änderung ist nachvollziehbar.
Klein.Buch erzwingt diese Regeln direkt im Code; Du kannst sie
nicht aus Versehen brechen.

## Hinweise (Inbox)

Die Liste der Mitteilungen, die Klein.Buch im Hintergrund für Dich
erzeugt: fälliges Backup, eingegangene E-Rechnung, fehlgeschlagener
Mail-Versand, abgelaufenes OAuth-Token. Du findest sie über das
Menü "Hinweise". Jeder Hinweis bleibt stehen, bis Du ihn als
gelesen markierst.

## Kleinbetragsrechnung

Eine Rechnung mit einem Gesamtbetrag bis 250 € (brutto). Hier
erlaubt das Gesetz vereinfachte Pflichtangaben. Klein.Buch erkennt
das automatisch und verwendet das passende Pflichtangaben-Schema.
Gesetzliche Grundlage: §33 UStDV.

## Kleinunternehmer

Eine Person mit Gewerbeschein oder freiberuflicher Tätigkeit, deren
Umsatz unter den gesetzlichen Grenzen bleibt (ab 2026: maximal
25.000 € im Vorjahr und voraussichtlich maximal 100.000 € im
laufenden Jahr). Wer Kleinunternehmer ist, weist auf Rechnungen
keine Umsatzsteuer aus, darf aber auch keine Vorsteuer abziehen.
Gesetzliche Grundlage: §19 UStG.

## KoSIT

Die Koordinierungsstelle für IT-Standards, die offizielle deutsche
Prüfstelle für XRechnung. Klein.Buch nutzt den KoSIT-Validator
intern, um sicherzustellen, dass jede ausgestellte XRechnung den
amtlichen Vorgaben entspricht. Du siehst den Validator nicht
direkt; er meldet sich nur, wenn etwas nicht passt.

## Leistungsdatum

Der Zeitpunkt oder Zeitraum, an dem Du die Leistung tatsächlich
erbracht hast. Pflichtangabe auf jeder Rechnung. Wenn Du die
Leistung am gleichen Tag erbringst und abrechnest, sind
Leistungs- und Belegdatum gleich. Bei Monats-Arbeiten (zum
Beispiel "Beratung im April 2026") ist es ein Zeitraum.

## Mustang

Ein Programm im Hintergrund, das die XRechnung-Datei und die
ZUGFeRD-PDF technisch erzeugt. Du siehst Mustang nicht direkt; es
läuft als Sidecar (siehe dort) zusammen mit Klein.Buch.

## Off-Site-Backup

Ein Backup, das nicht auf demselben Rechner liegt wie Deine Daten.
Cloud-Ordner (OneDrive, iCloud, Dropbox, Nextcloud) und SFTP-Server
zählen als Off-Site. Ohne Off-Site-Backup verlierst Du bei einem
Hardware-Schaden oder einem Diebstahl alle Daten. Klein.Buch warnt
Dich, wenn das letzte erfolgreiche Off-Site-Backup zu lange her
ist.

## Paket

Eine wiederverwendbare Leistung mit fertigem Titel, Beschreibung
und Preis. Du legst ein Paket einmal an und ziehst es per Klick in
jedes Angebot oder jede Rechnung. Wenn Du das Paket später
änderst, bleiben die alten Belege als historische Kopie unverändert
(das schützt Deine Buchführung vor nachträglicher Verfälschung).

## Passphrase

Das eine Passwort, mit dem Du Klein.Buch entsperrst. Es ist
gleichzeitig der Schlüssel zur verschlüsselten Datenbank und zu
allen Backups. Ohne Passphrase kommt niemand an Deine Daten
heran, auch Du nicht. Verlierst Du die Passphrase, sind alle Daten
und alle Backups unwiederbringlich weg. Es gibt absichtlich keine
Hintertür. Notiere die Passphrase in einem Passwort-Manager
außerhalb der App.

## PDF/A-3

Eine Variante des PDF-Formats, die für die Langzeit-Archivierung
gedacht ist. Bei ZUGFeRD steckt die XRechnung-Datei als
Datei-Anhang in einer PDF/A-3. Du brauchst Dich darum nicht zu
kümmern, Klein.Buch erzeugt das automatisch korrekt.

## Pflichtangaben

Die gesetzlich vorgeschriebenen Felder, die jede Rechnung enthalten
muss: vollständiger Name und Anschrift von Dir und vom Kunden,
Steuernummer, Rechnungsdatum, Rechnungsnummer,
Leistungsbeschreibung, Leistungsdatum, Betrag. Bei
Kleinunternehmern kommt der §19-Hinweis dazu. Klein.Buch prüft
Pflichtangaben vor jeder Festschreibung; ohne vollständige Angaben
geht die Festschreibung nicht durch. Gesetzliche Grundlage: §14
UStG.

## Privatbewegung

Eine Geld-Bewegung zwischen Deinem geschäftlichen Konto und Deinem
privaten Konto. Das ist keine echte Einnahme und keine echte
Ausgabe, sondern nur eine Umbuchung. Eine Privatbewegung verändert
den Gewinn nicht und taucht deshalb nicht in der EÜR auf. Es gibt
zwei Richtungen: Privatentnahme (Geld geht von geschäftlich nach
privat) und Privateinlage (Geld geht von privat nach
geschäftlich).

## Rechnungs-Eingang

Ein Ordner auf Deiner Festplatte, den Klein.Buch automatisch überwacht.
Wenn Du dort eine XRechnung-Datei oder eine ZUGFeRD-PDF ablegst, liest
Klein.Buch sie im Hintergrund ein und legt einen Eingangsbeleg in den
Kosten an. Optional, sinnvoll wenn Du regelmäßig E-Rechnungen per Mail
bekommst. Im Code und in internen Logs taucht das Feature unter dem
englischen Namen "Drop-Folder" auf, das ist derselbe Ordner. Richte
ihn in "Einstellungen" → "Rechnungs-Eingang" ein.

## Regelbesteuerung

Das normale Umsatzsteuer-Regime. Wer regelbesteuert ist, weist
Umsatzsteuer aus, führt sie ans Finanzamt ab und darf gezahlte
Umsatzsteuer aus Eingangsrechnungen als Vorsteuer abziehen. Wenn
Du als Kleinunternehmer freiwillig regelbesteuert sein willst,
heißt das "Verzicht auf §19". Die Bindung an die Regelbesteuerung
beträgt dann mindestens 5 Jahre.

## Wiederherstellen (Restore)

Das Zurückspielen eines Backups auf den jetzigen Rechner.
Klein.Buch erzeugt vor jeder Wiederherstellung automatisch ein
Sicherheits-Backup des aktuellen Stands, damit Du im Notfall
zurückwechseln kannst. Du brauchst die Passphrase, mit der das
Backup ursprünglich verschlüsselt wurde. Der englische Begriff
"Restore" bedeutet dasselbe und taucht in manchen technischen
Hinweisen noch auf.

## Roh-XML

Die unveränderte Original-Datei einer empfangenen elektronischen
Rechnung in maschinenlesbarer Form. Bei einer XRechnung ist das die
XML-Datei selbst; bei einer ZUGFeRD-Rechnung ist das die XML-Datei,
die als Anhang im PDF steckt. Klein.Buch zeigt Dir das Roh-XML auf
der Detailseite einer empfangenen Rechnung über den Button "Roh-XML
anzeigen". Praktisch für den Steuerberater oder eine
Betriebsprüfung, um die maschinenlesbaren Felder zu prüfen.

## SFTP

Ein Netzwerk-Protokoll für sicheres Kopieren von Dateien auf einen
Server (steht für "SSH File Transfer Protocol"). Klein.Buch kann
SFTP als Off-Site-Backup-Ziel nutzen, wenn Du einen eigenen Server
oder einen SFTP-fähigen Anbieter hast. Beim ersten Verbinden merkt
sich die App den Server-Schlüssel (den sogenannten Host Key),
damit ein Angreifer Dich nicht später auf einen falschen Server
umleiten kann.

## Sidecar

Ein eigenständiges Programm, das Klein.Buch im Hintergrund
mitstartet. Klein.Buch hat einen Sidecar für Mustang und den
KoSIT-Validator. Du siehst den Sidecar nicht direkt; er wird beim
App-Start automatisch hochgefahren und beim Beenden wieder
geschlossen. Wenn der Sidecar nicht hochkommt, kann Klein.Buch
keine E-Rechnungen erzeugen oder prüfen.

## SQLCipher

Die Verschlüsselungs-Schicht über der Datenbank. Sie sorgt dafür,
dass die Datenbank-Datei auf der Festplatte ohne Deine Passphrase
nicht lesbar ist. Selbst wenn jemand Deinen Rechner stiehlt oder
einen Backup-Ordner kopiert, kann er ohne die Passphrase nichts
damit anfangen.

## Steuernummer

Die Nummer, die Dir Dein Finanzamt zugeteilt hat (zum Beispiel
"123/456/78901"). Pflichtangabe auf jeder Rechnung. Wenn Du eine
Umsatzsteuer-Identifikations-Nummer hast (USt-IdNr, für Geschäfte
mit Kunden im EU-Ausland), darfst Du sie zusätzlich oder
alternativ angeben.

## Storno

Eine Gegenrechnung. Sie neutralisiert eine bereits festgeschriebene
Rechnung mit umgekehrtem Vorzeichen. Klein.Buch erzeugt sie als
eigenen Beleg mit eigener Nummer in der Form "ST-2026-0001". Das
nachträgliche Löschen einer Rechnung ist gesetzlich verboten;
Storno ist der einzige zulässige Weg, einen Fehler zu korrigieren.

## Verkäuferprofil

Deine eigenen Stammdaten in Klein.Buch: Firmen- oder
Berufs-Bezeichnung, Adresse, Steuernummer, Bankverbindung,
optional ein Logo. Klein.Buch druckt diese Daten auf jede Rechnung
und jedes Angebot. Du legst das Profil im Onboarding einmal an und
passt es in den Einstellungen an, falls sich etwas ändert.

## Verzicht auf §19 (Option zur Regelbesteuerung)

Die freiwillige Entscheidung, kein Kleinunternehmer mehr zu sein
und stattdessen normal Umsatzsteuer auszuweisen und abzuführen. Du
bindest Dich damit für mindestens 5 Jahre an die Regelbesteuerung
und kannst in dieser Zeit nicht zurück auf §19 wechseln.
Klein.Buch warnt Dich vor dieser Folge ausdrücklich, bevor Du den
Schalter umlegst.

## XRechnung

Das amtliche elektronische Rechnungsformat in Deutschland. Eine
XRechnung ist kein PDF, sondern eine maschinenlesbare XML-Datei.
Behörden und größere Kunden verlangen sie seit 2025. Klein.Buch
erzeugt in einem Arbeitsschritt beides: ein PDF zum Anschauen und
die XRechnung-Datei zum Verschicken.

## ZUGFeRD

Ein hybrides Rechnungsformat: ein PDF, in dem zusätzlich die
XRechnung-XML als Datei-Anhang steckt. Du verschickst eine einzige
Datei. Der Mensch sieht das PDF, die Buchhaltungs-Software beim
Empfänger liest automatisch die XML-Daten aus dem Anhang.
Klein.Buch kann ZUGFeRD ausstellen und ZUGFeRD-Eingangsrechnungen
lesen.

## ZUGFeRD-Profil

Eine Variante des ZUGFeRD-Formats. Das Profil sagt, wie vollständig
die Rechnungsdaten in der XML-Datei stehen. Klein.Buch akzeptiert
beim Empfang die Profile, die seit 2025 das Gesetz als gültige
E-Rechnung verlangt: `EN16931`, `EXTENDED` und `XRECHNUNG`. Die alten
Profile `MINIMUM` und `BASIC-WL` sind seit der E-Rechnungs-Pflicht
2025 keine gültigen Rechnungen mehr und werden abgelehnt. Wenn ein
Lieferant Dir eine MINIMUM- oder BASIC-WL-Datei schickt, bittest Du
ihn um eine richtige E-Rechnung oder trägst die Position selbst ein.

## Paragraphen aus den Steuergesetzen

Hier alle in Klein.Buch und in diesem Handbuch erwähnten
Paragraphen, jeweils mit einem Halbsatz, was sie regeln.

### §4 Abs. 3 EStG

Erlaubt die EÜR (Einnahmen-Überschuss-Rechnung) statt der
aufwendigeren Bilanz. Grundlage für Klein.Buch als Werkzeug für
Kleinunternehmer und kleine Selbständige.

### §11 EStG

Das Zufluss- und Abfluss-Prinzip. Eine Einnahme zählt im Jahr des
Geld-Eingangs, eine Ausgabe im Jahr der Zahlung. Daraus folgt die
Cash-Basis (siehe dort).

### §14 UStG

Listet die Pflichtangaben für Rechnungen auf. Klein.Buch prüft
diese Angaben vor jeder Festschreibung automatisch.

### §14c UStG

Regelt die Folge, wenn jemand fälschlich Umsatzsteuer ausweist:
Er schuldet sie dem Finanzamt, auch wenn er gar nicht
umsatzsteuerpflichtig wäre. Deshalb sperrt Klein.Buch im
§19-Modus alle Eingabefelder, die versehentlich Umsatzsteuer
ausweisen könnten.

### §19 UStG

Die Kleinunternehmer-Regelung. Wer unter den gesetzlichen
Umsatz-Grenzen bleibt, weist keine Umsatzsteuer aus und darf auch
keine Vorsteuer abziehen. Pflicht-Hinweis auf jeder Rechnung:
"Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen". Klein.Buch
setzt diesen Hinweis automatisch.

### §33 UStDV

Erlaubt Kleinbetragsrechnungen bis 250 € mit vereinfachten
Pflichtangaben.

### §147 AO

Schreibt die Aufbewahrungsfristen für Buchhaltungs-Unterlagen vor.
Für Rechnungen und alle steuerlich relevanten Belege gilt eine
Frist von 10 Jahren nach Ende des jeweiligen Geschäftsjahres.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
