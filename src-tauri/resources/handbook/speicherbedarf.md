---
slug: speicherbedarf
title: Speicher und Ressourcen
category: betrieb
order: 70
keywords: [speicher, arbeitsspeicher, ram, mem_limit, festplatte, speicherplatz, backup-größe, ressourcen]
---

# Speicher und Ressourcen

Dieses Kapitel erklärt, wie viel Arbeitsspeicher (RAM, der schnelle
Speicher, in dem Programme laufen) und wie viel Platz auf dem Speicher
Klein.Buch braucht, und mit welchen Einstellungen der Betreiber das
steuert. Der wichtigste Wert ist die Größe eines Backups. Fast alles
andere lässt sich daraus abschätzen.

## Wie groß ein Backup ist

Ein Backup enthält die Datenbank, das gesamte Beleg-Archiv sowie Logo und
Unterschrift, gepackt und verschlüsselt. Rechnungs-PDFs und Fotos von
Belegen lassen sich kaum weiter verkleinern. Ein Backup ist deshalb
ungefähr so groß wie Datenbank und Archiv zusammen.

Die aktuelle Größe siehst Du unter "Einstellungen" und "Backups" in der
Spalte "Größe" des Backup-Verlaufs oder im Backup-Protokoll. Sie wächst
mit jedem Beleg. Weil Belege zehn Jahre lang aufbewahrt werden müssen und
Klein.Buch nichts davon löscht, wird sie mit den Jahren nur größer. Schau
deshalb ab und zu nach und passe die Werte unten an.

## Arbeitsspeicher

Klein.Buch baut ein Backup vollständig im Arbeitsspeicher zusammen: erst
den gepackten Inhalt, dann die verschlüsselte Fassung, dann die fertige
Datei. Für einen kurzen Moment liegen alle drei gleichzeitig im
Speicher. Ein Backup braucht deshalb etwa das Dreifache seiner Größe an
Arbeitsspeicher. Dasselbe gilt für die Wiederherstellung (die Datei, ihre
verschlüsselte Kopie und der entschlüsselte Inhalt) und für die Prüfung
des Backups, das Klein.Buch vor einer Datenbank-Umstellung nach einem
Update anlegt. Backups laufen immer nacheinander, nie zwei gleichzeitig.

Dazu kommen:

1. Der Grundbedarf von Klein.Buch selbst. Das Compose-Beispiel rechnet
   mit 512 MiB.
2. Die Java-Komponente, die E-Rechnungen erzeugt und prüft. Sie darf je
   Lauf höchstens 512 MiB belegen, und es laufen höchstens zwei Läufe
   gleichzeitig.
3. Die Schlüssel-Berechnung bei Anmeldung und Backup (Argon2id) mit
   64 MiB, immer nur eine gleichzeitig.

Die Faustregel aus dem Compose-Beispiel lautet: 512 MiB Grundbedarf plus
das Dreifache des größten Backups. Die Grenze setzt der Betreiber in der
Compose-Datei mit `mem_limit`. Im Beispiel steht `mem_limit: 2g`. Nach
der Faustregel reicht das für Backups bis etwa 500 MiB. Werden die
Backups größer, hebt der Betreiber die Grenze an.

Ist die Grenze zu klein, beendet das Betriebssystem Klein.Buch mitten im
Backup oder in der Wiederherstellung, weil der Speicher ausgeht. Der
Container startet dann neu, und Klein.Buch ist gesperrt, bis sich jemand
anmeldet. Das Backup ist in diesem Fall nicht entstanden. Der Betreiber
erkennt das mit dem Befehl `docker inspect klein-buch`: Dort steht dann
`"OOMKilled": true` (OOM steht für "out of memory", also Speicher
erschöpft).

Der Ordner `/tmp` liegt im Compose-Beispiel ebenfalls im Arbeitsspeicher
(512 MiB). Klein.Buch legt große Zwischendateien deshalb bewusst nicht
dort ab, sondern im Daten-Ordner unter `/data/tmp`.

## Platz im Daten-Ordner

Im Daten-Ordner (`/data`) liegen:

1. Die Datenbank und das Beleg-Archiv. Beide wachsen mit jedem Beleg.
2. Die lokalen Backups in `/data/backups`. Solange mindestens ein
   externes Ziel funktioniert, behält Klein.Buch dort je ein Backup für
   die letzten 7 Tage mit Backup, die letzten 3 Monate und das letzte
   Jahr. Ohne funktionierendes externes Ziel sind es bis zu 30 Tage,
   12 Monate und 7 Jahre. Manuelle Backups, Sicherheits-Backups vor einer
   Wiederherstellung und die nach einer Wiederherstellung übernommenen
   Backups löscht Klein.Buch nie, sie kommen jedes Mal dazu. Während die
   automatischen Abläufe angehalten sind (nach einer Wiederherstellung),
   sortiert Klein.Buch nichts aus.
3. Zwischendateien in `/data/tmp`: Exporte bis zum Herunterladen (höchstens
   eine Stunde), hochgeladene Dateien bis zu ihrer Verarbeitung (ebenfalls
   höchstens eine Stunde) und die Kopie der Datenbank, die beim Backup
   kurz entsteht. Reste abgebrochener Läufe räumt Klein.Buch stündlich auf,
   sobald sie älter als sechs Stunden sind, und beim Containerstart vollständig.

Bevor Klein.Buch eine hochgeladene Datei annimmt, prüft es den freien
Platz. Nach dem Hochladen muss eine Reserve frei bleiben. Ihre Größe
setzt die Umgebungsvariable `KLEIN_BUCH_MIN_FREE_BYTES`, Standard 2 GiB.
Reicht der Platz nicht, lehnt Klein.Buch den Upload mit "Zu wenig freier
Speicherplatz auf dem Server" ab.

Eine Wiederherstellung braucht zusätzlich das Doppelte des entpackten
Backup-Inhalts an freiem Platz, plus die Reserve, plus Platz für das
Sicherheits-Backup, das Klein.Buch vorher anlegt. Die größte erlaubte
Backup-Datei und zugleich die größte erlaubte entpackte Menge setzt
`KLEIN_BUCH_MAX_RESTORE_BYTES`, Standard 4 GiB. Ist ein Backup größer,
hebt der Betreiber diesen Wert an, bevor er es einspielt.

Beide Werte sind reine Byte-Zahlen ohne Einheit: `2147483648` für 2 GiB,
`4294967296` für 4 GiB, `8589934592` für 8 GiB. Eine Angabe wie `8G`
versteht Klein.Buch nicht und bricht den Start ab. In der Compose-Datei
stehen sie unter `environment:`, zum Beispiel:

```
      KLEIN_BUCH_MAX_RESTORE_BYTES: "8589934592"
```

Läuft der Daten-Ordner trotzdem voll, kann Klein.Buch nicht einmal mehr
die lokale Kopie eines Backups schreiben. Dann entsteht gar kein Backup,
und auch eine Datenbank-Umstellung nach einem Update unterbleibt.

## Platz auf den Backup-Zielen

Auf jedem Ordner-Ziel behält Klein.Buch je ein Backup für die letzten
30 Tage mit Backup, die letzten 12 Monate und die letzten 7 Jahre. Auf
Dauer sind das höchstens 49 Backups (oft weniger, weil ein Backup
zugleich für Tag, Monat und Jahr steht), dazu alle manuellen Backups und
Sicherheits-Backups, die Klein.Buch nie löscht.

Auf SFTP-Servern löscht Klein.Buch bewusst nichts: Eine kompromittierte
Instanz soll die externen Kopien nicht vernichten können. Dort wächst der
Bestand mit jedem Backup — mit einem Tages-Backup und einigen
Festschreibungen am Tag sind das grob 400 bis 1 000 Backups im Jahr. Bei
300 MiB je Backup wären das 120 bis 300 GiB im Jahr. Der Betreiber des
SFTP-Servers räumt deshalb selbst auf, am besten nach demselben Muster
(zum Beispiel die letzten 30 Tage vollständig, danach ein Backup je Monat,
nach einem Jahr eines je Jahr), oder er nutzt Snapshots des Servers mit
eigener Aufbewahrung. Steht für den SFTP-Benutzer eine Speichergrenze
(Quota), schlägt die Spiegelung fehl, sobald sie erreicht ist; das zeigt
das Backup-Protokoll.

## Ein Rechenbeispiel

Angenommen, Datenbank und Archiv sind zusammen 300 MiB groß. Dann ist ein
Backup ebenfalls etwa 300 MiB groß.

Arbeitsspeicher: 512 MiB plus dreimal 300 MiB ergibt rund 1,4 GiB. Die
2 GiB aus dem Compose-Beispiel reichen.

Daten-Ordner: 300 MiB für Datenbank und Archiv, elf lokale Backups mit
zusammen rund 3,2 GiB, dazu manuelle Backups und die Reserve von 2 GiB.
Für eine Wiederherstellung kommen kurzzeitig das hochgeladene Backup, das
Sicherheits-Backup und zweimal der entpackte Inhalt dazu, zusammen etwa
1,2 GiB.

Ordner-Ziel: auf Dauer bis zu 49 Backups mit rund 14,4 GiB, dazu die
manuellen Backups.

## Weitere Grenzen im Compose-Beispiel

`pids_limit: 256` begrenzt die Zahl der Prozesse und Threads im
Container. `stop_grace_period: 90s` ist die Zeit, die Docker Klein.Buch
beim Anhalten gibt. Klein.Buch selbst bleibt darunter: höchstens 30
Sekunden für offene Verbindungen, dann höchstens 30 Sekunden für laufende
Arbeiten wie ein Festschreiben oder ein Backup, dann das Schließen der
Datenbank. Ein Backup, das in dieser Zeit nicht fertig wird, bricht ab,
ohne eine halbe Datei zu hinterlassen; das nächste Tages-Backup holt es
nach. Die Angaben unter `logging:` sorgen dafür, dass das Container-Log
höchstens fünf Dateien mit je 10 MB belegt.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
