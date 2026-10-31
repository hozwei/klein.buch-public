---
slug: volumes-und-rechte
title: Volumes und Rechte
category: betrieb
order: 30
keywords: [volume, volumes, rechte, uid, chown, dateisystem, nfs, smb, inputs, seed-manifest, neu, snapshot, data, browser, datev]
---

# Volumes und Rechte

Ein Volume ist ein Ordner auf Deinem Server, den Docker in den Container
einblendet. Klein.Buch speichert nichts Dauerhaftes im Container selbst:
Alles, was bleiben soll, liegt in Volumes. Ein Container lässt sich
jederzeit wegwerfen und neu anlegen, die Volumes nicht. Fehlt beim Containerstart
ein Volume, bricht Klein.Buch ab, statt Daten an einer Stelle zu
schreiben, die mit dem Container verschwinden würde.

## Die drei Bereiche

`/data` enthält alles, was Deine Buchführung ausmacht: die verschlüsselte
Datenbank (`klein-buch.sqlite`), das Archiv der festgeschriebenen Belege
(`archive/`), die lokalen Backups (`backups/`), Logo und Unterschrift
(`branding/`), den zweiten Faktor und den Geräteschlüssel (`auth/`),
Zwischendateien (`tmp/`) und den Instanz-Marker `.klein-buch-instance`.
Der Marker ist eine kleine Datei, an der Klein.Buch erkennt, dass
Datenbank und Ordner zusammengehören.

`/inputs` enthält die Dateien, die Menschen pflegen: PDF-Vorlagen
(`pdf-templates/`), Mail-Vorlagen (`mail-templates/`), die AfA-Tabellen
und Fachgrundlagen (`specs/`) sowie `branding/` und `examples/`.
Klein.Buch füllt den Ordner beim ersten Start aus dem Image und hält ihn
danach aktuell, ohne Deine Änderungen anzufassen (siehe unten).

Unter `/backup-targets/` liegt je externem Backup-Ziel ein eigener
Ordner, und jeder davon ist ein eigenes Volume, zum Beispiel die
Freigabe eines NAS (eines Netzwerkspeichers) oder der Ordner eines
Sync-Programms für Deinen Cloud-Speicher. In den Backup-Einstellungen
zeigt Klein.Buch die eingebundenen Ordner zur Auswahl an. Die erste Ebene
unter `/backup-targets` legt Klein.Buch nie selbst an: Was dort nicht
eingebunden ist, gibt es nicht.

`/data` und `/inputs` müssen zwei getrennte Volumes sein, und keiner der
beiden Ordner auf dem Server darf im anderen liegen. Klein.Buch prüft beim
Start, dass beide wirklich eingebunden sind.

Drei Einstellungen liegen nicht auf dem Server, sondern im Browser: den
Kontenrahmen sowie die Berater- und Mandantennummer für den
DATEV-Export merkt sich jeder Browser selbst. Sie stehen deshalb in
keinem Backup. Ohne die beiden Nummern startet der DATEV-Export nicht,
und das Steuerberater-Paket entsteht ohne DATEV-Stapel. In einem anderen Browser, auf einem anderen Gerät oder
nach einem Wechsel der Adresse in `KLEIN_BUCH_PUBLIC_URL` trägst Du sie
auf der Seite "EÜR-Export" einmal neu ein.

## Rechte

Klein.Buch läuft unter der Benutzer- und Gruppennummer aus der Zeile
`user` der Compose-Datei, im Beispiel `1000:1000`; ohne diese Zeile als
`65532:65532`. Dieser Benutzer muss `/data`, `/inputs` und jedes
Backup-Ziel lesen und beschreiben dürfen:

```
sudo chown -R 1000:1000 /srv/klein-buch/data /srv/klein-buch/inputs
```

Klein.Buch selbst setzt strenge Rechte: Archivdateien sind nach dem
Schreiben nur lesbar (Recht `0400`), der Ordner `auth/` gehört allein
dem Klein.Buch-Benutzer, das Geheimnis des zweiten Faktors ebenso.
Ändere diese Rechte nicht.

Als `root` startet Klein.Buch nicht. Beim Start legt es zur Probe eine
schreibgeschützte Datei im Archiv an und versucht, sie trotzdem zu
öffnen. Gelingt das, wäre der Schreibschutz Deiner festgeschriebenen
Belege wirkungslos, und Klein.Buch bricht mit "Selbsttest im Archiv …
gescheitert" ab. Genau das passiert als `root`, aber auch auf
Dateisystemen, die das Recht `0400` nicht umsetzen.

## Nur lokale Dateisysteme für Datenbank und Archiv

Leg `/data` auf ein lokales Linux-Dateisystem mit echten Dateirechten und
Hardlinks, zum Beispiel ext4, XFS, Btrfs oder ZFS. Ein Hardlink ist ein
zweiter Name für dieselbe Datei; Klein.Buch nutzt ihn, damit eine neue
Archivdatei nie eine vorhandene überschreiben kann. Nicht geeignet sind
Netzwerkfreigaben (NFS, SMB oder CIFS) und Dateisysteme von USB-Sticks
und Speicherkarten (exFAT, FAT). Dafür gibt es drei Gründe.

Die Datenbank braucht Dateisperren und gemeinsam genutzten Speicher, die
Netzwerkfreigaben nicht zuverlässig liefern. Im schlimmsten Fall wird
die Datenbank beschädigt. Der Schreibschutz des Archivs beruht auf dem
Recht `0400`; auf exFAT oder mit den Zugriffslisten mancher
NFS-Freigaben greift er nicht, und Klein.Buch verweigert dann den Start.
Und Klein.Buch wartet nach jedem wichtigen Schreiben, bis die Daten
wirklich auf der Platte stehen (fsync). Manche Netzwerkfreigaben melden
das, bevor es stimmt.

Für die Backup-Ziele unter `/backup-targets/` ist eine Netzwerkfreigabe
dagegen in Ordnung. Dort schreibt Klein.Buch nur fertige, verschlüsselte
Kopien, erst unter einem vorläufigen Namen und dann umbenannt.

Eine sinnvolle Ergänzung sind Snapshots. Das sind schreibgeschützte
Momentaufnahmen eines Ordners, wie sie ZFS oder Btrfs anbieten. Unter
Linux kann derselbe Benutzer eine Archivdatei trotz `0400` löschen.
Klein.Buch bemerkt das beim nächsten Lesen und bei der monatlichen
Prüfung des Archivs, zurückholen lässt sich die Datei aber nur aus einem
Backup oder einem Snapshot, an den der Klein.Buch-Benutzer nicht
herankommt. Snapshots ersetzen die Backups von Klein.Buch nicht.

## /inputs braucht eine eigene Sicherung

Die Backups von Klein.Buch enthalten Datenbank, Archiv und Branding
(Logo und Unterschrift), aber nicht `/inputs`. Sichere `/inputs` selbst,
zum Beispiel mit den übrigen Sicherungen Deines Servers oder per
Snapshot. Das ist vor allem wichtig, wenn Du Vorlagen angepasst hast.
Nach einer Wiederherstellung prüft Klein.Buch, ob die PDF-Vorlagen, die
Deine Firmendaten, offene Entwürfe und Abo-Rechnungen nennen, in
`/inputs` vorhanden sind, und meldet fehlende unter "Hinweise".

## Wie Klein.Buch /inputs aktuell hält

Bei jedem Containerstart vergleicht Klein.Buch `/inputs` mit den Dateien, die das
Image mitbringt. Dafür führt es die Datei `/inputs/.seed-manifest.json`.
Sie merkt sich zu jeder Datei eine Prüfsumme (SHA-256, eine Art
Fingerabdruck des Inhalts) der Fassung, die Klein.Buch zuletzt dort
abgelegt hat. Daraus folgt für jede Datei:

- Fehlt sie, legt Klein.Buch sie an. Das gilt auch, wenn Du eine
  mitgelieferte Datei gelöscht hast.
- Hast Du sie nicht verändert und das Image bringt eine neue Fassung,
  ersetzt Klein.Buch sie und schreibt einen Hinweis.
- Hast Du sie verändert, fasst Klein.Buch sie nie an. Die neue Fassung
  legt es einmal daneben als `<Name>.neu` ab, zum Beispiel
  `pdf-templates/default.typ.neu`, und schreibt einen Hinweis.

Die Hinweise erscheinen nach der nächsten Anmeldung unter "Hinweise",
weil die Datenbank beim Containerstart noch verschlossen ist. Vergleiche
Deine Datei mit der `.neu`-Fassung und übernimm, was Du brauchst. Danach
kannst Du die `.neu`-Datei löschen, egal ob Du die neue Fassung
übernommen oder Deine eigene behalten hast: Klein.Buch merkt sich in der
Manifest-Datei, dass es diese Fassung schon angeboten hat, und legt sie
nicht wieder an. Erst wenn ein späteres Image eine weitere Fassung
mitbringt, liegt wieder eine `.neu`-Datei daneben, mit neuem Hinweis.

Ändere oder lösche `.seed-manifest.json` nicht. Ohne sie hält Klein.Buch
jede Datei, die vom Image abweicht, für von Dir verändert und legt
überall `.neu`-Dateien an.

## Ein Volume löschen

Eine Funktion "Alles löschen" gibt es im Server-Betrieb nicht. Wer
`/data` löscht, löscht damit auch die Unterlagen, die Du aufbewahren
musst (Bücher und Aufzeichnungen 10 Jahre, Belege 8 Jahre; Klein.Buch
hält alles 10 Jahre vor). Diese Verantwortung liegt bei Dir als
Betreiber. Willst Du neu anfangen, leg eine neue Instanz mit einem neuen,
leeren Ordner an und bewahre den alten samt Backups auf.

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
