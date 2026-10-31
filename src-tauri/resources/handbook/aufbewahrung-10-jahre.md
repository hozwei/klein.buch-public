---
slug: aufbewahrung-10-jahre
title: Aufbewahrung 10 Jahre
category: recht-und-steuern
order: 330
keywords: [aufbewahrung, frist, 10 jahre, §147 ao, archiv, prüfung, beleg]
---

# Aufbewahrung 10 Jahre

Steuerlich relevante Unterlagen musst Du in Deutschland mehrere Jahre
aufbewahren. Die Fristen regelt §147 AO (Abgabenordnung): 10 Jahre
für Bücher und Aufzeichnungen (bei Dir etwa die EÜR, das
Anlagenverzeichnis und die Buchungen in Klein.Buch), seit 2025 8 Jahre
für Buchungsbelege wie Rechnungen und Quittungen (§147 Abs. 3 AO und
§14b UStG in der Fassung des Vierten Bürokratieentlastungsgesetzes)
und 6 Jahre für Geschäftsbriefe. Die Frist beginnt mit Ablauf des
Kalenderjahres, in dem die Unterlage entstanden ist (Rechnungsdatum,
Quittungsdatum, Buchungs-Eintrag) — also am 1. Januar des Folgejahres.
Klein.Buch bewahrt alles einheitlich 10 Jahre auf, weil Belege und
Buchungen in derselben Datenbank stehen; länger aufzubewahren ist
erlaubt.

## Beispiel

Eine Rechnung mit Rechnungsdatum Mai 2026 ist 2026 entstanden.
Die Aufbewahrungs-Frist beginnt am 1. Januar 2027; für die Rechnung
als Buchungsbeleg endet sie am 31. Dezember 2034, für die Buchung in
Deiner EÜR am 31. Dezember 2036. Klein.Buch hält beides bis
mindestens Ende 2036 vor.

Auch wenn Du Dein Gewerbe vorher abmeldest, läuft die Frist
weiter. Du musst die Belege auch dann noch zugänglich haben, wenn
Du gar nicht mehr selbständig bist.

## Was alles unter die Frist fällt

1. Ausgangsrechnungen (Deine eigenen)
2. Eingangsrechnungen (Kosten-Belege)
3. Quittungen (Bar-Käufe, Tankquittungen)
4. Bank-Auszüge oder Kontoumsatz-Listen
5. AfA-Berechnungen und Anlagenverzeichnis
6. EÜR und Steuererklärung
7. Geschäfts-Korrespondenz, soweit steuerlich relevant (für reine
   Handels- und Geschäftsbriefe gilt nach §147 Abs. 3 AO eine
   kürzere 6-Jahres-Frist; Klein.Buch behandelt seine Inhalte
   einheitlich mit 10 Jahren)
8. Sonstige Aufzeichnungen, die für die Besteuerung relevant sind
   (Fahrtenbuch, Reisekosten-Aufstellungen)

Klein.Buch deckt die Punkte 1, 2, 5 und 6 direkt ab. Punkt 3
deckst Du ab, indem Du die Quittungen als Beleg-Anhang an die
Kosten-Position hängst. Punkte 4, 7, 8 musst Du außerhalb von
Klein.Buch sicherstellen, zum Beispiel über Dein Online-Banking
(Auszug-Archiv) oder einen geordneten Ordner.

## Wo Klein.Buch die Belege speichert

Klein.Buch legt jede festgeschriebene Rechnung, jede XRechnung-XML
und jeden hochgeladenen Eingangsbeleg im Archiv ab. Das Archiv liegt
im Daten-Volume Deiner Instanz, im Container unter `/data/archive`.
Klein.Buch setzt jede Datei nach dem Schreiben auf reine Lese-Rechte
und merkt sich ihren SHA-256-Prüfwert, eine Art digitalen
Fingerabdruck. Bei jedem Lesen vergleicht es die Datei mit diesem
Wert, und eine monatliche Prüfung meldet fehlende oder veränderte
Dateien. Die Lese-Rechte schützen vor versehentlichem Überschreiben.
Vor dem Löschen durch jemanden, der direkt auf dem Server arbeitet,
schützen sie nicht. Deshalb gehören verschlüsselte externe Backups
und, wenn Dein Server das kann, Snapshots des Dateisystems dazu.

Beim Backup wird der Archiv-Ordner mitgesichert. Solange Deine
Backups laufen, sind Deine Belege also auch außerhalb des
Original-Speicherorts vorhanden.

## Cloud-Backup und Ausland

Die GoBD verlangt, dass die Daten im EU-Wirtschaftsraum bleiben.
Speicherung außerhalb der EU ist möglich, muss aber dem Finanzamt
mitgeteilt werden (§146 Abs. 2a AO). Microsoft 365 Business bietet
mit dem passenden Geo-Setting EU-Speicherorte; Google Workspace
und Dropbox haben EU-Speicherort in einigen Geschäfts-Tarifen als
Zusatz-Option. Bei iCloud lässt sich der Speicherort nicht
zuverlässig festlegen — für GoBD-relevante Daten ist iCloud daher
nicht empfohlen. Klein.Buch greift selbst nicht auf Cloud-Anbieter
zu; das macht ein Sync-Programm auf dem Server, dessen Ordner Du als
Backup-Ziel unter `/backup-targets` einbindest. Prüf beim Anbieter
Deiner Wahl, dass EU-Speicher konfiguriert ist. Dasselbe gilt für
den Server, auf dem Klein.Buch selbst läuft, wenn Du ihn mietest.

## Was Klein.Buch nicht tun darf

Klein.Buch bietet keine Lösch-Funktion für festgeschriebene
Belege. Es gibt absichtlich keinen Knopf, der eine einzelne
Rechnung aus der Datenbank entfernt. Es gibt auch keine Funktion,
die alle Daten auf einmal löscht. Eine Instanz endet nur, wenn Du als
Betreiber ihr Daten-Volume auf dem Server löschst. Dann trägst Du
selbst die Verantwortung dafür, dass die Unterlagen, die Du noch
aufbewahren musst (Bücher und Aufzeichnungen 10 Jahre, Buchungsbelege
8 Jahre, Geschäftsbriefe 6 Jahre), anderswo erhalten bleiben, zum
Beispiel in einem Export oder einem Backup samt Passphrase.

Das wirkt am Anfang umständlich. Es schützt Dich aber
verlässlich: Bei einer späteren Prüfung kannst Du dem Finanzamt
zeigen, dass Du die Aufbewahrungsfristen von Anfang an
architektonisch nicht brechen konntest.

## Nach Ablauf der Frist

Klein.Buch löscht auch nach zehn Jahren nichts von selbst. Die Frist
verlängert sich nämlich, solange die Unterlagen für eine noch offene
Steuerfestsetzung gebraucht werden, etwa während einer Außenprüfung
oder eines Einspruchs (§147 Abs. 3 AO). Das kann Klein.Buch nicht
beurteilen, und eine Automatik könnte gerade benötigte Belege
vernichten. Ist die Frist für Dich sicher abgelaufen, entscheidest Du
selbst.

Die Anonymisierung eines Kontakts (Kapitel "DSGVO-Auskunft und
Anonymisierung") entfernt dabei nur einen Teil: die Kontaktdaten
selbst, den Empfänger in verworfenen Rechnungs- und
Angebots-Entwürfen, Bezeichnung und Notiz seiner Abo-Vorlagen und die
Bezeichnung seiner Kosten-Abos. Was Klein.Buch unveränderlich
aufbewahrt, bleibt auch nach der Frist: die ausgestellten Rechnungen
und Storno-Belege und die fertiggestellten Angebote mit dem damaligen
Empfänger, ihre PDF- und XML-Dateien im Archiv, das E-Mail-Protokoll
mit den Empfängeradressen und das Audit-Protokoll.
Einzelne Belege oder eine einzelne Person lassen sich daraus nicht
löschen. Den ganzen Datenbestand beendest Du, indem Du die Instanz
stilllegst und ihr Volume samt Backups löschst; vorher sicherst Du, was
noch aufbewahrt werden muss, zum Beispiel jüngere Jahre in einem
Export.

## Was beim Server-Wechsel passieren muss

Wenn Du mit Klein.Buch auf einen neuen Server umziehst:

1. Den Vorlagen-Ordner `/inputs` auf den neuen Server übernehmen,
   falls Du dort etwas geändert hast. Er ist nicht Teil des Backups.
2. Auf dem neuen Server Klein.Buch als Container einrichten und im
   Browser die Einrichtung abschließen.
3. Die neueste Backup-Datei (Endung `.kbk`) aus einem Deiner
   externen Backup-Ziele holen.
4. Unter "Einstellungen → Backups" im Abschnitt "Wiederherstellen"
   die Datei hochladen.
5. Das Backup-Passwort eingeben, also die Passphrase, mit der das
   Backup verschlüsselt wurde.
6. Backup-Ziele prüfen und bei Bedarf neu einrichten.

Die Aufbewahrungs-Frist läuft beim Umzug unverändert weiter. Der
Archiv-Inhalt kommt mit dem Backup mit. Die Einzelheiten stehen im
Kapitel "Notfall: Instanz neu aufsetzen und wiederherstellen".

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
