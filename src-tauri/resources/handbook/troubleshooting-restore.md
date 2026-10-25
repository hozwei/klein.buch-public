---
slug: troubleshooting-restore
title: Fehler beim Wiederherstellen
category: troubleshooting
order: 550
keywords: [wiederherstellen, restore, fehler, passphrase, backup, sicherheits-backup, upload, kbk]
---

# Fehler beim Wiederherstellen

Beim Wiederherstellen spielst Du ein Backup in Deine Klein.Buch-Instanz
zurück. Klein.Buch prüft das Backup gründlich, bevor es irgendetwas
ersetzt. Die meisten Fehler zeigen sich deshalb, bevor sich an Deinen
Daten etwas ändert. Und sobald die eigentliche Wiederherstellung
beginnt, gibt es ein Sicherheits-Backup des vorherigen Stands. Hier die
typischen Fehler.

## Passphrase wird nicht akzeptiert

Die Meldung lautet "Authentifizierung fehlgeschlagen (falsche Passphrase
oder beschädigtes Backup)".

Mögliche Ursachen:

1. Du verwendest Deine heutige Passphrase, das Backup wurde aber mit
   einer älteren verschlüsselt.
2. Ein Tippfehler, häufig bei langen Passphrasen.
3. Ein anderes Tastatur-Layout als beim Festlegen der Passphrase (zum
   Beispiel vertauschte Tasten y und z).
4. Die Datei ist beschädigt (siehe nächster Abschnitt).

Lösung: Versuch die Passphrase, die galt, als das Backup erstellt wurde.
Das Erstellungsdatum steht im "Backup-Verlauf" und im Dateinamen. Bei
einer falschen Passphrase schreibt Klein.Buch nichts und behält die
hochgeladene Datei. Du kannst also einfach eine andere Passphrase
eingeben und noch einmal auf "Wiederherstellen" klicken, ohne die Datei
erneut hochzuladen.

Hast Du die alte Passphrase nicht mehr, lässt sich dieses Backup nicht
einspielen. Es gibt absichtlich keine Hintertür.

## Backup-Datei ist beschädigt oder unvollständig

Meldungen wie "Backup-Datei: kein MAGIC-Header", "ungültiges Backup" oder
"Content-Hash stimmt nicht" bedeuten: Die Datei ist keine vollständige
Klein.Buch-Sicherung. Meist wurde sie beim Kopieren oder Herunterladen
abgeschnitten oder beschädigt. Auch die Meldung zur falschen Passphrase
kann bei einer beschädigten Datei erscheinen.

Lösung: Hol die Datei noch einmal frisch vom Ziel. Vergleiche ihre Größe
mit dem Wert im "Backup-Protokoll". Weicht sie ab, ist beim Kopieren
etwas schiefgegangen. Hilft das nicht, nimm ein anderes Backup, zum
Beispiel das vom Vortag. Eine Wiederherstellung öffnet Backup-Dateien nur
lesend. Ein misslungener Versuch beschädigt also kein Backup.

## Das Backup stammt aus einer anderen Version

"Backup hat Schema-Version …, diese Version kennt höchstens …" heißt:
Das Backup kommt aus einer neueren Klein.Buch-Version als der, die gerade
läuft. Eine ältere Version kann neuere Daten nicht lesen. Lösung: Der
Betreiber aktualisiert Klein.Buch mindestens auf die Version, aus der
das Backup stammt (siehe Kapitel "[Updates und
Rückfall](/help/updates-und-rueckfall)").

"Backup hat Schema-Version …; eingespielt werden Backups ab Version 30"
heißt: Das Backup ist älter als Klein.Buch 2026.5. Solche Backups nimmt
der Server nicht an.

Backups aus älteren Versionen ab 2026.5 sind dagegen kein Problem.
Klein.Buch spielt sie ein und stellt die Datenbank bei der nächsten
Anmeldung auf das aktuelle Format um. Vorher legt es selbst ein Backup
an.

## "Die Archivpfade im Backup passen nicht"

Klein.Buch merkt sich zu jedem Beleg, wo die Datei im Archiv liegt. Im
Container ist das immer der Ordner `/data/archive`. Passen die Angaben im
Backup nicht dazu, lehnt Klein.Buch es ab. Das passiert bei Backups der
früheren Desktop-Fassung, sobald darin festgeschriebene Belege stehen,
und bei Backups einer Instanz, die mit einem anderen Daten-Ordner
betrieben wurde. Einen Umzug von der Desktop-Fassung in den Server gibt
es nicht. Wer die Desktop-Fassung produktiv genutzt hat, bewahrt den
Bestand mit dieser Fassung auf.

## "verwaiste Verweise" oder "mehr als einen festgeschriebenen Storno-Beleg"

Klein.Buch prüft die Datenbank im Backup so, wie es sie später beim
Entsperren prüft. "Die Datenbank im Backup enthält verwaiste Verweise"
heißt: Ein Eintrag verweist auf einen Datensatz, den es im Backup nicht
gibt, etwa nach einer Bearbeitung der Datenbank von Hand. "… mehr als
einen festgeschriebenen Storno-Beleg" heißt: Zu einer Rechnung gibt es
zwei festgeschriebene Stornos; diese Version könnte die Datenbank nicht
mehr entsperren. In beiden Fällen lehnt Klein.Buch das Backup ab, bevor
es etwas austauscht; Deine Instanz bleibt, wie sie ist. Nimm ein anderes
Backup; die Einzelheiten stehen im Server-Log unter "Restore-Prüfung eines
Backups" (nicht unter "Entsperren").

## Backup zu groß oder zu wenig Platz

"Backup zu groß für den Restore" heißt: Der entpackte Inhalt ist größer
als erlaubt oder auf dem Server ist nicht genug Platz. Klein.Buch
verlangt beim Wiederherstellen etwa das Doppelte des entpackten Inhalts
an freiem Platz plus eine Reserve, weil es den Inhalt erst vorbereitet
und dann an seinen Platz kopiert. Schon beim Hochladen kann Klein.Buch
ablehnen: "Anfrage zu groß." heißt, die Datei ist größer als erlaubt.
"Zu wenig freier Speicherplatz auf dem Server" heißt, nach dem Hochladen
bliebe weniger als die Reserve frei. Die genauen Zahlen stehen dann im
Server-Log.

Lösung für den Betreiber: Platz im Daten-Ordner schaffen. Die Grenzen
lassen sich über die Umgebungsvariablen `KLEIN_BUCH_MAX_RESTORE_BYTES`
(größtes erlaubtes Backup, Standard 4 GiB) und `KLEIN_BUCH_MIN_FREE_BYTES`
(Reserve an freiem Platz, Standard 2 GiB) anpassen. Einzelheiten stehen
im Kapitel "[Speicher und Ressourcen](/help/speicherbedarf)".

Bricht das Hochladen ab, bevor es 100 Prozent erreicht, kann auch eine
Größengrenze im Reverse-Proxy schuld sein (dem Programm, das die
verschlüsselte Verbindung zum Browser herstellt und Anfragen an
Klein.Buch weiterreicht). Diese Grenze muss mindestens so groß sein wie
die Backup-Datei. Siehe Kapitel "[Reverse-Proxy und
TLS](/help/reverse-proxy-und-tls)".

## Der Server kommt nach der Wiederherstellung nicht zurück

Nach "Wiederherstellung vorbereitet" beendet sich der Server, damit die
Container-Verwaltung ihn neu startet. Beim Start spielt er das Backup
ein. Ist er nach drei Minuten nicht zurück, meldet die Seite "Der Server
ist nach drei Minuten nicht zurück. Bitte den Container prüfen und die
Seite neu laden."

Mögliche Ursachen:

1. Der Container startet nach dem Beenden nicht von selbst neu. Die
   Neustart-Regel muss `unless-stopped` oder `always` sein, wie im
   Compose-Beispiel. Mit `on-failure` startet Docker einen Container,
   der sich ordnungsgemäß beendet hat, nicht neu. Dann startet der
   Betreiber ihn von Hand, zum Beispiel mit `docker compose up -d`.
2. Der Start dauert bei einem großen Archiv einfach länger. Warte noch
   etwas und lade die Seite neu.
3. Der Start ist gescheitert. Die Ursache steht im Server-Log (siehe
   Kapitel "[Fehlersuche im Container](/help/fehlersuche-im-container)").
   Bricht das Einspielen mittendrin ab, legt Klein.Buch die
   ursprünglichen Daten beim nächsten Start wieder an ihren Platz.

## Nach dem Neustart klappt die Anmeldung nicht

Nach der Wiederherstellung gilt die Passphrase des eingespielten
Backups, nicht mehr die bisherige. Den Code liefert weiter dieselbe
Authenticator-App wie vorher, denn diese Verbindung ist nicht Teil des
Backups.

Erscheint stattdessen "Entsperren abgebrochen: Vor der
Datenbank-Umstellung ließ sich kein Backup schreiben", stammte das
Backup aus einer älteren Version und vor der Umstellung fehlte Platz
oder Schreibrecht. Lies dazu den passenden Abschnitt im Kapitel
"[Backup-Fehler](/help/troubleshooting-backup)".

## Zurück zum Stand vor der Wiederherstellung

Hat die Wiederherstellung geklappt, aber das Ergebnis ist nicht das, was
Du wolltest, spielst Du das Sicherheits-Backup zurück. Es steht im
"Backup-Verlauf" mit dem Grund "Vor einer Wiederherstellung" und ist mit
der Passphrase verschlüsselt, die vor der Wiederherstellung galt.

## Vorbeugen

1. Halte jede Passphrase, die Du je verwendet hast, im Passwort-Manager
   fest. Nur mit ihr lassen sich die Backups aus dieser Zeit einspielen.
2. Probiere das Wiederherstellen regelmäßig in einer Test-Instanz aus,
   wie im Kapitel "[Backup prüfen](/help/backup-pruefen)" beschrieben.
3. Kopiere eine Backup-Datei vor dem Hochladen vollständig auf Dein
   Gerät, statt sie direkt aus einem Ordner zu nehmen, der gerade noch
   synchronisiert wird.

---

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.10*
