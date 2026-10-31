---
slug: glossar
title: Glossar
category: glossar
order: 10
keywords: [glossar, fachbegriffe, definitionen, abkürzungen, kleinunternehmer, §19, gobd, eür, afa, xrechnung, zugferd, passphrase, daten-passwort, app-code, totp, container, instanz, sidecar, rechnungseingang, roh-xml, zugferd-profil, anschaffung, verwerfen, storno-beleg, firmendaten, e-mail-protokoll, aktion bestätigen, leitweg-id, leistungszeitraum, vorausrechnung, einheit, gegenbuchung, zahlungsbuch, zahlung stornieren, rückstand, nachholen, vorgemerkter entwurf, abgleich]
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
eine Abo-Vorlage an und sagst Klein.Buch, in welchem Rhythmus daraus
neue Rechnungen werden sollen. Du erreichst sie über "Rechnungen" und
"Abo-Rechnungen". Je Vorlage wählst Du einen Modus: "Nur Entwurf",
"Automatisch ausstellen" oder "Ausstellen + senden" (dann schickt
Klein.Buch die Rechnung direkt ab, sobald sie fällig ist). Für
regelmäßige Ausgaben gibt es das Kosten-Abo (siehe dort).

## Abgang

Das Ausscheiden einer Anschaffung aus dem Betrieb: Verkauf,
Verschrottung oder Privatentnahme bzw. Geschenk. Du buchst ihn auf
der Seite der Anschaffung über das Menü "Mehr" und "Veräußern /
Entsorgen". Vor dem Buchen zeigt Klein.Buch den Restbuchwert (siehe
dort) zum gewählten Datum; mit "Abgang bestätigen" wird gebucht.

## Aktion bestätigen

Der Dialog, mit dem Du eine geschützte Aktion noch einmal bestätigst,
zum Beispiel einen Export, eine Wiederherstellung oder eine Änderung
der Bankverbindung. Er nennt die Aktion im Titel und fragt Dein
Daten-Passwort und den App-Code (oder einen Einmal-Code) ab. Danach
gilt die Bestätigung 5 Minuten lang. "Abbrechen" ändert nichts.

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
einträgst. Den Satz legst Du unter "Einstellungen" und
"Anfahrtspauschale" fest.

## Anlage

Ein teurer Gegenstand, den Du nicht im Kaufjahr komplett absetzt,
sondern über mehrere Jahre verteilt (siehe AfA). Typische Anlagen
sind Notebook, Werkstattmaschine, Geschäftswagen. In der Oberfläche
heißen der Bereich "Anschaffungen" und jedes einzelne Stück
"Anschaffung" (mit Nummer "AV-…"). Das Wort "Anlage" steht dort nur
als Steuerbegriff, zum Beispiel beim Knopf "Als Anlage übernehmen" in
den Kosten oder bei der "Anlage EÜR".

## Anschaffung

So heißt in Klein.Buch ein Wirtschaftsgut, das Du über mehrere Jahre
abschreibst; der Bereich in der Navigation heißt "Anschaffungen".
Siehe Anlage und AfA.

## Anonymisierung (DSGVO)

Das Löschen aller persönlichen Daten eines Kontakts, entweder auf
Deine eigene Entscheidung oder auf Anforderung der betroffenen
Person. Klein.Buch ersetzt Name, Adresse und Notizen durch leere
Felder und archiviert den Kontakt. Die alten Rechnungen bleiben als
Buchhaltungs-Beleg unverändert erhalten (das ist gesetzliche Pflicht
laut GoBD, kein Bug). Siehe auch DSGVO.

## App-Code (TOTP)

Der sechsstellige Code aus einer Authenticator-App auf Deinem Handy.
Die App erzeugt alle 30 Sekunden einen neuen Code. TOTP steht für
"Time-based One-Time Password", also ein zeitbasiertes
Einmal-Passwort. Bei jeder Anmeldung gibst Du den App-Code zusammen
mit Deiner Passphrase ein. So reicht eine erratene oder mitgelesene
Passphrase allein nicht, um an Deine Daten zu kommen. Für den Fall,
dass das Handy verloren geht, bekommst Du bei der Einrichtung zehn
Einmal-Codes. Jeder davon ersetzt genau einmal den App-Code.

## Aufbewahrungsfrist

Die gesetzliche Pflicht, bestimmte Belege eine Mindestzeit
aufzuheben: in Deutschland 10 Jahre für Bücher und Aufzeichnungen,
seit 2025 8 Jahre für Buchungsbelege wie Rechnungen, 6 Jahre für
Geschäftsbriefe, jeweils ab Ende des Kalenderjahres. Klein.Buch
bewahrt alles einheitlich 10 Jahre auf. Klein.Buch hat absichtlich keine Lösch-Funktion
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

Der Ordner oder Server, in den Klein.Buch die Backups schreibt. Eine
lokale Sicherung im Daten-Volume läuft immer automatisch mit. Dazu
richtest Du externe Ziele ein, auch mehrere: Ordner unter
`/backup-targets`, die Du als Betreiber in den Container einbindest
(zum Beispiel eine NAS-Freigabe oder den Ordner eines
Sync-Programms für einen Cloud-Speicher), oder einen SFTP-Server
(siehe SFTP). Mindestens ein externes Ziel ist Pflicht, sonst warnt
Klein.Buch Dich.

## Belegdatum

Das Datum, das oben auf der Rechnung steht; im Formular heißt das
Feld "Rechnungsdatum" (beim Angebot "Angebotsdatum"). In der Regel
der Tag, an dem Du die Rechnung schreibst und ausstellst. Nicht zu
verwechseln mit dem Leistungsdatum (siehe dort). Bei Kosten ist das
"Beleg-Datum" das Datum auf der Eingangsrechnung.

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

## Container und Instanz

Ein Container ist ein abgeschlossenes Programm-Paket, das ein Server
startet. Klein.Buch wird als fertiges Paket (das sogenannte Image)
verteilt; daraus startet Dein Server den Container. Eine Instanz ist
eine laufende Klein.Buch-Installation mit ihren eigenen Daten. Eine
Instanz gehört genau einem Unternehmen. Wer den Container auf dem
Server einrichtet, aktualisiert und sichert, ist der Betreiber. Das
bist meist Du selbst.

## DATEV-Export

Ein Daten-Format, das praktisch jeder Steuerberater versteht.
Klein.Buch erzeugt Dir auf Knopfdruck einen DATEV-Stapel mit allen
Buchungen des Geschäftsjahres (Datei `EXTF_Buchungsstapel_…csv`), den
Dein Steuerberater in sein System importiert. Standard-Kontenrahmen ist
SKR03. Die Berater- und die Mandantennummer, die Dir Dein Steuerberater
nennt, sind Pflicht; Du trägst sie im EÜR-Export ein, und Dein Browser
merkt sie sich (sie stehen nicht in der Datenbank und nicht im Backup).

## DSGVO (Datenschutz-Grundverordnung)

Das EU-Datenschutz-Gesetz. Es gibt jeder Person das Recht zu
erfahren, welche Daten Du über sie speicherst (Auskunft, Art. 15),
und unter bestimmten Bedingungen das Recht, dass Du diese Daten
löschst (Anonymisierung, Art. 17). Klein.Buch unterstützt beide
Funktionen auf der Seite des jeweiligen Kontakts.

## E-Mail-Protokoll

Die Liste aller Versandversuche mit Empfänger, Ergebnis und Antwort
des Mailservers, auch der gescheiterten. Einträge lassen sich nicht
ändern oder löschen. Du findest sie unter "Einstellungen" und
"E-Mail-Protokoll".

## EÜR (Einnahmen-Überschuss-Rechnung)

Die einfache Steuer-Aufstellung für Kleinunternehmer und kleine
Selbständige: Einnahmen minus Ausgaben. Du brauchst sie einmal im
Jahr für Deine Steuererklärung. Klein.Buch erstellt sie auf
Knopfdruck; in der Navigation heißt der Bereich "Steuer (EÜR)".
Gesetzliche Grundlage: §4 Abs. 3 EStG.

## ELSTER

Das offizielle Online-Portal des Finanzamts. Dort gibst Du Deine
Steuererklärung digital ab. Klein.Buch exportiert die EÜR in einem
Format, das Du direkt in das ELSTER-Formular übertragen kannst
(diese Funktion heißt in der App "Ausfüllhilfe").

## Einheit

Die Mengeneinheit einer Position (Stunde, Stück, Monat, Pauschale
…). Klein.Buch zeigt sie im Klartext und speichert dahinter den
genormten Code, den die E-Rechnung verlangt. Eine alte, frei
eingetippte Einheit wie "Pauschal" meldet das Formular als Fehler am
Feld und schlägt die passende vor.

## Factory Reset

Das vollständige Zurücksetzen auf Werkszustand, bei dem alle Daten
gelöscht werden. Diese Funktion gibt es in Klein.Buch absichtlich
nicht: Über das Netz wäre eine Totallöschung die gefährlichste
einzelne Aktion. Wer neu anfangen will, legt als Betreiber eine neue
Instanz mit leerem Daten-Volume an. Wer ein altes Volume löscht,
trägt selbst die Verantwortung für die 10-jährige
Aufbewahrungspflicht.

## Festschreibung

Das endgültige Abschließen eines Belegs. Ab diesem Moment kannst
Du die zentralen Felder (Beträge, Kunde, Datum) nicht mehr ändern.
Eine festgeschriebene Rechnung lässt sich nur noch durch einen
Storno neutralisieren. Diese Sperre kommt aus der GoBD und schützt
Dich vor Vorwürfen bei einer Steuerprüfung. In der Oberfläche heißt
der Schritt je nach Beleg anders: Eine Rechnung wird "ausgestellt",
ein Angebot "fertiggestellt" (Status "Fertiggestellt", der Versand
ist ein eigener Schritt). Kosten und Privatbewegungen sind schon beim
Erfassen festgeschrieben; deshalb heißen die Knöpfe dort "Kosten
festschreiben" und "Privatbewegung festschreiben" und fragen vorher
nach.

## Gegenbuchung

Die Buchung, mit der Klein.Buch eine falsch erfasste Zahlung
storniert: derselbe Betrag mit umgekehrtem Vorzeichen, mit Datum und
Konto der falschen Zahlung, dazu ein Grund. Sie steht im Zahlungsbuch
der Rechnung als "Gegenbuchung" mit dem Vermerk "Storno der Zahlung
vom …"; die Zahlung selbst bleibt stehen und trägt den Vermerk
"Storniert". Du startest sie in der Karte "Zahlungen" der Rechnung über
"Mehr" und "Zahlung stornieren …". Sie wirkt am ursprünglichen
Zahldatum, weil die falsche Zahlung nie ein Zufluss war (§ 11 EStG).
Eine Rückzahlung an den Kunden ist keine Gegenbuchung, sondern gehört
zum Storno der Rechnung (siehe Storno).

## Gesamtumsatz

Die Summe Deiner Umsätze in einem Kalenderjahr, nach der sich die
Grenzen der Kleinunternehmer-Regelung richten (§ 19 Abs. 2 UStG):
nach vereinnahmten Entgelten, also nach Zahlungseingang, netto und ohne
Umsätze mit Anlagevermögen. Klein.Buch zeigt den Gesamtumsatz des
Vorjahres und des laufenden Jahres im Abschnitt "Steuerstatus" unter
Einstellungen → "Meine Firmendaten" und meldet, wenn eine Grenze
erreicht oder überschritten ist. Es zählt nur die in Klein.Buch
erfassten Zahlungen; eine stornierte Zahlung und ihre Gegenbuchung
zählen beide nicht.

## Geschäftsjahr

In Klein.Buch immer das Kalenderjahr (1. Januar bis 31. Dezember).
Jede Rechnung, jede Kostenposition und jede AfA-Buchung gehört zu
genau einem Geschäftsjahr. Die Rechnungs-Nummern starten in jedem
neuen Geschäftsjahr von vorn (zum Beispiel "RE-2026-0001").

## Geschäftsjahr-Abschluss

Das endgültige Festschreiben eines kompletten Geschäftsjahres.
Danach kannst Du in dem abgeschlossenen Jahr nichts mehr ändern,
auch keine alten Belege nachtragen. Vorher solltest Du das
Steuerberater-Paket (siehe dort) erzeugt und die EÜR übermittelt
haben. Abgeschlossene Jahre stehen auf der Seite "Geschäftsjahr" im
Abschlussprotokoll.

## GoBD (Grundsätze ordnungsmäßiger Buchführung)

Die Regeln, nach denen das Finanzamt Buchhaltungs-Software
akzeptiert. Die wichtigsten Punkte: Belege sind nach der
Festschreibung unveränderlich, Storno ersetzt Löschung, alles wird
10 Jahre aufbewahrt, jede Änderung ist nachvollziehbar.
Klein.Buch erzwingt diese Regeln direkt im Code; Du kannst sie
nicht aus Versehen brechen.

## Hinweise

Die Liste der Mitteilungen, die Klein.Buch im Hintergrund für Dich
erzeugt: fälliges Backup, gescheitertes Backup-Ziel, fehlgeschlagener
Mail-Versand, neue Version (wenn Du die Versionsprüfung eingeschaltet
hast). Du findest sie in der Navigation unter "Hinweise"; daneben
steht die Zahl der offenen Hinweise. Jeder Hinweis bleibt stehen, bis
Du ihn abhakst ("Abhaken" oder "Alle abhaken"); abgehakte Hinweise
zeigt "Abgehakte anzeigen". Welche Hinweise erscheinen, stellst Du
unter "Erinnerungen einstellen" ein.

## Kleinbetragsrechnung

Eine Rechnung mit einem Gesamtbetrag bis 250 € (brutto). Hier
erlaubt das Gesetz vereinfachte Pflichtangaben. Klein.Buch erkennt
das automatisch und verwendet das passende Pflichtangaben-Schema.
Eine Steuernummer oder USt-IdNr braucht Klein.Buch trotzdem, weil
auch zur Kleinbetragsrechnung eine XRechnung entsteht. Gesetzliche
Grundlage: §33 UStDV.

## Kleinunternehmer

Eine Person mit Gewerbeschein oder freiberuflicher Tätigkeit, deren
Umsatz unter den gesetzlichen Grenzen bleibt (seit 2025: höchstens
25.000 € im Vorjahr und höchstens 100.000 € im laufenden Jahr; wer
die 100.000 € im laufenden Jahr überschreitet, verliert §19 ab diesem
Umsatz). Wer Kleinunternehmer ist, weist auf Rechnungen
keine Umsatzsteuer aus, darf aber auch keine Vorsteuer abziehen.
Gesetzliche Grundlage: §19 UStG.
Ob Du an einem Tag Kleinunternehmer bist, steht in Klein.Buch im
Verlauf des Steuerstatus (siehe "Steuerstatus").

## Korrigiert neu erfassen

Der Knopf auf einer stornierten Kostenposition. Er öffnet das
Formular für neue Kosten mit den Werten des Originals, damit Du nach
einem Storno nur noch den Fehler korrigierst. Es entsteht eine neue,
eigenständige Kostenposition.

## Kosten-Abo

Eine Vorlage für regelmäßige Ausgaben wie Miete oder Software, unter
"Kosten" und "Kosten-Abos". Im Modus "Automatisch" bucht
Klein.Buch die Kosten am Stichtag selbst, im Modus "Erinnerung"
erscheint ein Hinweis "Abo fällig" und Du buchst per Knopf.

## KoSIT

Die Koordinierungsstelle für IT-Standards, die offizielle deutsche
Prüfstelle für XRechnung. Klein.Buch nutzt den KoSIT-Validator
intern, um sicherzustellen, dass jede ausgestellte XRechnung den
amtlichen Vorgaben entspricht. Du siehst den Validator nicht
direkt; er meldet sich nur, wenn etwas nicht passt.

## Leistungsdatum

Der Zeitpunkt, an dem Du die Leistung erbringst. Pflichtangabe auf
der Rechnung (§14 UStG). Wenn Du die Leistung am gleichen Tag
erbringst und abrechnest, sind Leistungs- und Belegdatum gleich;
Klein.Buch belegt das Feld deshalb mit dem Rechnungsdatum vor, bis Du
es änderst. Bei laufenden Arbeiten gibst Du statt dessen einen
Leistungszeitraum an (siehe dort). Liegt das Datum nach dem
Rechnungsdatum, ist es eine Vorausrechnung (siehe dort). Nach dem
Leistungsdatum, bei einem Zeitraum nach dessen Ende, richtet sich
auch der Steuerstatus der Rechnung (siehe "Steuerstatus").

## Leistungszeitraum

Die Zeitspanne, in der Du eine Leistung erbringst, mit Beginn und
Ende (zum Beispiel "Beratung vom 01.04.2026 bis 30.04.2026"). Du
wählst im Rechnungs-Formular unter "Leistung" entweder "Datum" oder
"Zeitraum (von–bis)", nie beides. Das Ende darf nicht vor dem Beginn
liegen; für einen ganzen Kalendermonat genügt der Erste bis Letzte
des Monats (§ 31 Abs. 4 UStDV). Auf dem PDF steht "Leistungszeitraum:
TT.MM.JJJJ – TT.MM.JJJJ", in der E-Rechnung der Rechnungszeitraum
(BG-14). Abo-Rechnungen tragen den Abrechnungszeitraum automatisch,
ein Storno-Beleg den Zeitraum der Original-Rechnung. Nach dem
Ausstellen ist er unveränderlich.

## Leitweg-ID

Die Kennung, mit der eine Behörde E-Rechnungen an die richtige
Stelle leitet. Nur Rechnungen an öffentliche Auftraggeber brauchen
sie. Sie besteht aus Grobadressierung, optional Feinadressierung und
einer zweistelligen Prüfziffer, durch Bindestriche getrennt (zum
Beispiel 04011000-12345-67). Du trägst sie im Entwurf ein (Feld
"Leitweg-ID (optional)"), der Entwurf speichert sie, beim Ausstellen
kommt sie als Käuferreferenz in die E-Rechnung; danach ist sie
unveränderlich. Bei allen anderen Kunden bleibt das Feld leer.

## Menü "Mehr"

Das Menü oben auf einer Seite, in dem seltene und gefährliche
Aktionen stehen (zum Beispiel "PDF herunterladen" oder
"Stornieren"). Gefährliche Aktionen stehen dort unten. Auf dem
Smartphone liegen dort auch die Nebenaktionen; sichtbar bleibt nur
der nächste Schritt.

## Mustang

Ein Programm im Hintergrund, das die XRechnung-Datei und die
ZUGFeRD-PDF technisch erzeugt. Du siehst Mustang nicht direkt; es
läuft als Sidecar (siehe dort) zusammen mit Klein.Buch.

## Off-Site-Backup

Ein Backup, das nicht auf demselben Speicher liegt wie Deine Daten.
In Klein.Buch heißen sie "externe Ziele": eingebundene Ordner unter
`/backup-targets` (zum Beispiel eine NAS-Freigabe oder der Ordner
eines Sync-Programms für einen Cloud-Speicher) und SFTP-Server. Ohne
Off-Site-Backup verlierst Du bei einem Hardware-Schaden oder einem
Diebstahl des Servers alle Daten. Klein.Buch warnt Dich, wenn kein
externes Ziel eingerichtet ist, wenn ein Ziel zuletzt gescheitert ist
oder wenn das letzte erfolgreiche Backup zu lange her ist.

## Paket

Eine wiederverwendbare Leistung mit fertigem Titel, Beschreibung
und Preis. Du legst ein Paket einmal an und fügst es mit "+ Paket"
in jedes Angebot oder jede Rechnung ein. Jede Änderung speichert
Klein.Buch als neue Version; eine frühere Version holst Du mit
"Wiederverwenden" zurück, auch das ergibt eine neue Version. Die alten
Belege bleiben als historische Kopie unverändert (das schützt Deine
Buchführung vor nachträglicher Verfälschung). Pakete ordnest Du in
Kategorien, die Du umbenennen und sortieren kannst.

## Passphrase

Das eine Passwort, mit dem Du Klein.Buch entsperrst. In der
Oberfläche heißt es "Daten-Passwort", beim Wiederherstellen eines
Backups "Backup-Passwort". Bei der Anmeldung gibst Du es zusammen mit
dem App-Code ein (siehe dort). Es ist gleichzeitig der Schlüssel zur
verschlüsselten Datenbank und zu allen Backups. Ohne Passphrase kommt niemand an Deine Daten
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
geht die Festschreibung nicht durch. Bei einem Rechnungs-Entwurf
zeigt die Karte "Ausstellen" schon beim Öffnen, was noch fehlt.
Gesetzliche Grundlage: §14 UStG.

## Privatbewegung

Eine Geld-Bewegung zwischen Deinem geschäftlichen Konto und Deinem
privaten Konto. Das ist keine echte Einnahme und keine echte
Ausgabe, sondern nur eine Umbuchung. Eine Privatbewegung verändert
den Gewinn nicht und taucht deshalb nicht in der EÜR auf. Es gibt
zwei Richtungen: Privatentnahme (Geld geht von geschäftlich nach
privat) und Privateinlage (Geld geht von privat nach
geschäftlich). Der Bereich heißt "Privatbewegungen". Eine
Privatbewegung ist mit "Privatbewegung festschreiben" sofort
festgeschrieben; einen Fehler korrigierst Du mit einer Gegenbuchung
in die andere Richtung.

## Rechnungseingang

Der Weg, auf dem empfangene E-Rechnungen in Klein.Buch kommen: Du
lädst die XRechnung-Datei oder die ZUGFeRD-PDF im Browser hoch, über
"Kosten" und "E-Rechnung importieren". Klein.Buch liest die Angaben
aus ("Einlesen und prüfen"), prüft sie mit KoSIT und legt mit
"Kosten festschreiben" nach einer Rückfrage einen Eingangsbeleg in
den Kosten an. Einen überwachten Ordner (früher
"Drop-Folder" genannt) gibt es nicht.

## Regelbesteuerung

Das normale Umsatzsteuer-Regime. Wer regelbesteuert ist, weist
Umsatzsteuer aus, führt sie ans Finanzamt ab und darf gezahlte
Umsatzsteuer aus Eingangsrechnungen als Vorsteuer abziehen. Wenn
Du als Kleinunternehmer freiwillig regelbesteuert sein willst,
heißt das "Verzicht auf §19". Die Bindung an die Regelbesteuerung
beträgt dann fünf Kalenderjahre einschließlich des Wirkungsjahres.
Fällt §19 weg, weil Dein Umsatz eine Grenze überschreitet (über
25.000 € im Vorjahr, über 100.000 € im laufenden Jahr), ist das kein
Verzicht und bindet nicht.

## Wiederherstellen (Restore)

Das Zurückspielen eines Backups in Deine Klein.Buch-Instanz.
Klein.Buch erzeugt vor jeder Wiederherstellung automatisch ein
Sicherheits-Backup des aktuellen Stands, damit Du im Notfall
zurückwechseln kannst. Du brauchst die Passphrase, mit der das
Backup ursprünglich verschlüsselt wurde. Der englische Begriff
"Restore" bedeutet dasselbe und taucht in manchen technischen
Hinweisen noch auf.

## Restbuchwert

Der Wert einer Anschaffung, der nach den bisherigen Abschreibungen
noch nicht abgesetzt ist. Bei einem Abgang zählt er in der EÜR als
Ausgabe; ein Verkaufserlös darüber ist Gewinn. Klein.Buch zeigt ihn
vor dem Buchen eines Abgangs zum gewählten Datum an.

## Roh-XML

Die unveränderte Original-Datei einer empfangenen elektronischen
Rechnung in maschinenlesbarer Form. Bei einer XRechnung ist das die
XML-Datei selbst; bei einer ZUGFeRD-Rechnung ist das die XML-Datei,
die als Anhang im PDF steckt. Klein.Buch zeigt Dir das Roh-XML auf
der Detailseite einer empfangenen Rechnung über den Button "Roh-XML
anzeigen". Praktisch für den Steuerberater oder eine
Betriebsprüfung, um die maschinenlesbaren Felder zu prüfen.

## Rückstand (Abo)

Termine einer Abo-Rechnung oder eines Kosten-Abos, die ausgefallen
sind, weil Klein.Buch gesperrt war, der Container nicht lief oder das
Abo pausiert war. Bei mehr als drei offenen Terminen und beim
Fortsetzen einer pausierten Reihe fragst Du über "Rückstand …":
"Nachholen" (höchstens 24 Termine) oder "Ab heute fortsetzen". Der
Dialog nennt Anzahl und Daten der offenen Termine und, womit es nach
"Ab heute fortsetzen" weitergeht; bei mehr als 24 Terminen gibt es nur
"Ab heute fortsetzen".

## SFTP

Ein Netzwerk-Protokoll für sicheres Kopieren von Dateien auf einen
Server (steht für "SSH File Transfer Protocol"). Klein.Buch kann
SFTP als Off-Site-Backup-Ziel nutzen, wenn Du einen eigenen Server
oder einen SFTP-fähigen Anbieter hast. Beim ersten Verbinden merkt
sich die App den Server-Schlüssel (den sogenannten Host Key),
damit ein Angreifer Dich nicht später auf einen falschen Server
umleiten kann.

## Sidecar

Ein eigenständiges Hilfsprogramm in Java, das Klein.Buch im
Container bei Bedarf startet. Klein.Buch hat einen Sidecar für
Mustang und den KoSIT-Validator. Du siehst den Sidecar nicht direkt;
er läuft für jeden Vorgang kurz an und beendet sich danach wieder.
Wenn der Sidecar nicht antwortet, kann Klein.Buch keine E-Rechnungen
erzeugen oder prüfen.

## SQLCipher

Die Verschlüsselungs-Schicht über der Datenbank. Sie sorgt dafür,
dass die Datenbank-Datei im Daten-Volume ohne Deine Passphrase
nicht lesbar ist. Selbst wenn jemand das Daten-Volume oder einen
Backup-Ordner kopiert, kann er ohne die Passphrase nichts damit
anfangen.

## Steuerberater-Paket

Ein ZIP-Paket für Deinen Steuerberater mit Deckblatt, "Anlage
EÜR"-PDF, DATEV-Stapel, ELSTER-Ausfüllhilfe, Einzelaufstellungen und
den Belegen. Du erzeugst es unter "Steuer (EÜR)", "Für die
Steuererklärung exportieren" und "Paket herunterladen". Wie jeder
Export lässt es sich genau einmal herunterladen.

## Steuernummer

Die Nummer, die Dir Dein Finanzamt zugeteilt hat (zum Beispiel
"123/456/78901"). Pflichtangabe auf jeder Rechnung. Wenn Du eine
Umsatzsteuer-Identifikations-Nummer hast (USt-IdNr, für Geschäfte
mit Kunden im EU-Ausland), darfst Du sie zusätzlich oder
alternativ angeben.

## Steuerstatus

Ob Du an einem Tag Kleinunternehmer nach §19 UStG oder
regelbesteuert bist. Klein.Buch führt ihn als Verlauf unter
Einstellungen → "Meine Firmendaten", Abschnitt "Steuerstatus": Jeder
Eintrag hat ein Wirkungsdatum und einen Grund (Stand zu Beginn,
Verzicht auf §19, Wegfall durch eine Umsatzgrenze, Rückkehr). Ohne
Eintrag gilt die Kleinunternehmer-Regelung. Einträge werden nie
gelöscht oder geändert; ein irrtümlicher Eintrag wird mit Grund
widerrufen. Jeder Beleg zählt mit dem Status seines Datums
(Rechnungen: Leistungsdatum bzw. Ende des Leistungszeitraums, Kosten:
Belegdatum), beim Festschreiben
bleibt der Status einer Rechnung unverändert. Mehr im Kapitel
"Kleinunternehmer-Regelung".

## Storno

Eine Gegenrechnung. Sie neutralisiert eine bereits ausgestellte
Rechnung mit umgekehrtem Vorzeichen. Klein.Buch erzeugt sie als
eigenen Beleg mit eigener Nummer in der Form "ST-2026-0001"; in der
Oberfläche heißt er "Storno-Beleg" (auf dem PDF "Stornorechnung").
Du startest ihn auf der Rechnung über das Menü "Mehr" und
"Stornieren"; vorher fragt Klein.Buch nach. Das nachträgliche Löschen
einer Rechnung ist gesetzlich verboten; Storno ist der einzige
zulässige Weg, einen Fehler zu korrigieren. Kosten stornierst Du
ebenfalls über "Mehr" und "Stornieren", mit Grund; sie stehen danach
als "Storniert" in der Liste, ein eigener Beleg entsteht dabei nicht.
Einen noch nicht ausgestellten Entwurf stornierst Du nicht, Du
verwirfst ihn (siehe Verwerfen). Eine falsch erfasste Zahlung
stornierst Du nicht über die Rechnung, sondern einzeln mit "Zahlung
stornieren …" (siehe Gegenbuchung); die Rechnung bleibt dabei gültig.

## Verkäuferprofil

Deine eigenen Firmendaten in Klein.Buch: Firmen- oder
Berufs-Bezeichnung, Adresse, Steuernummer bzw. USt-IdNr, Telefon und
E-Mail. In der Oberfläche heißt die Seite "Meine Firmendaten" (unter
"Einstellungen"), in Meldungen kurz "Firmendaten". Die Bankverbindung
pflegst Du unter "Einstellungen" und "Konten", Logo und Unterschrift
unter "Rechnungs-Layout". Klein.Buch druckt diese Daten auf jede
Rechnung und jedes Angebot. Solange Angaben fehlen, die das Ausstellen
verlangt, erinnert Dich die Startseite daran.

## Verwerfen

Was Du mit einem Rechnungs- oder Angebots-Entwurf machst, den Du
nicht mehr brauchst: Menü "Mehr" und "Entwurf verwerfen", mit einem
Grund. Der Entwurf bleibt mit dem Status "Verworfen" sichtbar und
wird nicht gelöscht; bei Rechnungen bleibt die schon vergebene Nummer
verbraucht. "Stornieren" gibt es nur für festgeschriebene Belege.

## Verzicht auf §19 (Option zur Regelbesteuerung)

Die freiwillige Entscheidung, kein Kleinunternehmer mehr zu sein
und stattdessen normal Umsatzsteuer auszuweisen und abzuführen
(§ 19 Abs. 3 UStG). Der Verzicht wirkt ab dem 1. Januar des Jahres,
für das er gilt, und bindet Dich fünf Kalenderjahre einschließlich
dieses Jahres; in dieser Zeit kannst Du nicht zurück auf §19
wechseln (Verzicht ab 2026: Rückkehr frühestens zum 01.01.2031).
Du trägst ihn im Abschnitt "Steuerstatus" mit "Statuswechsel
eintragen …" ein; der Dialog nennt das Ende der Bindung, bevor Du
bestätigst. Ein Wegfall durch eine Umsatzgrenze ist kein Verzicht und
bindet nicht.

## Vorausrechnung

Eine Rechnung, die Du stellst, bevor Du die Leistung erbringst: Das
Leistungsdatum oder das Ende des Leistungszeitraums liegt nach dem
Rechnungsdatum. Das ist erlaubt (§14 Abs. 4 Satz 1 Nr. 6 UStG); Du
gibst den voraussichtlichen Leistungszeitpunkt an. Klein.Buch zeigt
dann am Feld den Hinweis "Rechnung vor der Leistung", speichert und
stellt die Rechnung aber ganz normal aus. Das Rechnungsdatum selbst
darf nicht in der Zukunft liegen. Für den Steuerstatus zählt der
angegebene Leistungszeitpunkt.

## Vorgemerkter Entwurf

Ein Rechnungs- oder Angebots-Entwurf aus einem wiederhergestellten
Backup, dessen Nummer vor der Wiederherstellung schon festgeschrieben
war. Er ist ein älterer Stand eines Belegs, den Du schon verschickt
hast. Du passt ihn an das Original an ("Bearbeiten") und bestätigst den
Abgleich unter "Einstellungen" → "Backups" → "Nummernkreise"; erst
dann lässt er sich ausstellen bzw. fertigstellen. Änderst Du ihn nach
der Bestätigung, ist er wieder vorgemerkt; darauf weist der Editor vor
dem Speichern hin.

## XRechnung

Das amtliche elektronische Rechnungsformat in Deutschland. Eine
XRechnung ist kein PDF, sondern eine maschinenlesbare XML-Datei.
Behörden und größere Kunden verlangen sie seit 2025. Klein.Buch
erzeugt in einem Arbeitsschritt beides: ein PDF zum Anschauen und
die XRechnung-Datei zum Verschicken.

## Zahlungsbuch

Die Liste aller Buchungen auf eine Rechnung: Zahlungen und
Gegenbuchungen, jede mit Zahldatum, Betrag, Konto und Notiz bzw. Grund.
Klein.Buch hängt nur an; eine Buchung wird nie geändert oder gelöscht
(GoBD). "Bezahlt", "Offen" und der Status der Rechnung ("Teilbezahlt",
"Bezahlt") ergeben sich aus der Summe des Zahlungsbuchs. Du siehst es
in der Karte "Zahlungen" der Rechnung.

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

### §13b UStG

Reverse-Charge: Bei bestimmten Leistungen, vor allem von Anbietern
im Ausland, schuldet der Empfänger die Umsatzsteuer statt des
Lieferanten. Das gilt auch für Kleinunternehmer; sie melden die
Steuer an und zahlen sie, ohne Vorsteuerabzug. Klein.Buch markiert
solche Kosten, berechnet die Steuer aber nicht (Kapitel "Kosten
erfassen").

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

Schreibt die Aufbewahrungsfristen für Buchhaltungs-Unterlagen vor:
10 Jahre für Bücher und Aufzeichnungen, seit 2025 8 Jahre für
Buchungsbelege wie Rechnungen, 6 Jahre für Geschäftsbriefe, jeweils
ab Ende des Kalenderjahres.

### §153 AO

Verlangt, eine abgegebene Steuererklärung zu berichtigen, wenn Du
nachträglich erkennst, dass sie unrichtig ist und dadurch zu wenig
Steuer festgesetzt werden kann. Klein.Buch verweist darauf, wenn eine
falsch erfasste Zahlung in einem schon abgeschlossenen Geschäftsjahr
liegt und sich deshalb nicht mehr stornieren lässt.

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
