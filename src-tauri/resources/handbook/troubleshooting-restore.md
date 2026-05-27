---
slug: troubleshooting-restore
title: Fehler beim Wiederherstellen
category: troubleshooting
order: 550
keywords: [wiederherstellen, restore, fehler, passphrase, backup, sicherheits-backup]
---

# Fehler beim Wiederherstellen

Das Wiederherstellen spielt ein Backup zurück in die laufende
Klein.Buch-Instanz. Wenn dabei etwas schiefgeht, schützt Dich das
vorab automatisch angelegte Sicherheits-Backup vor dauerhaftem
Daten-Verlust. Hier die typischen Fehler.

## Passphrase wird nicht akzeptiert

Symptom: "Passphrase falsch" beim Start der Wiederherstellung.

Mögliche Ursachen:

1. Du verwendest die heutige Passphrase, das Backup wurde aber mit
   einer älteren Passphrase verschlüsselt.
2. Tippfehler (häufig bei langen Passphrasen).
3. Tastatur-Layout-Wechsel zwischen Backup-Erzeugung und
   Wiederherstellung.

Lösung: Versuche die Passphrase, die zum Zeitpunkt der
Backup-Erzeugung aktuell war. Klein.Buch zeigt im Backup-Listing
das Erzeugungs-Datum. Wenn Du die Passphrase damals geändert hast,
nutze die jeweils zu dem Backup passende Variante.

Wenn Du die alte Passphrase nicht mehr hast, kannst Du dieses
spezielle Backup nicht mehr wiederherstellen. Die App lässt
absichtlich keine Hintertür zu.

## Backup-Datei ist beschädigt

Symptom: "Backup-Datei kann nicht entpackt werden" oder
"Integritäts-Prüfung der Hülle fehlgeschlagen".

Wahrscheinliche Ursache: Die Datei wurde beim Übertragen oder beim
Cloud-Sync unterbrochen oder beschädigt. Bei OneDrive passiert das
selten, aber gelegentlich.

Lösung: Wenn Du das Backup aus einer Cloud holst, lade es noch
einmal frisch herunter (oder erzwinge eine neue Synchronisation).
Wenn Du das Backup aus einem lokalen Ordner nutzt, vergleiche die
Datei-Größe und das Datum mit dem Wert, den Klein.Buch beim
Erzeugen im Backup-Protokoll vermerkt hat. Weicht eines davon ab,
ist die Datei beim Übertragen beschädigt worden.

Wenn die Datei dauerhaft beschädigt ist, nutze ein anderes Backup
aus der Retention-Liste (zum Beispiel die Vorwoche).

## Schema-Version-Mismatch

Symptom: "Backup hat ein anderes Schema (vXX); aktuelle App
erwartet vYY".

Bedeutung: Das Backup stammt aus einer älteren Klein.Buch-Version
mit einem älteren Datenbank-Schema. Die Migrations-Schritte für
das neue Schema sind im Backup nicht enthalten.

Bedeutung: Das Backup stammt aus einer anderen Klein.Buch-Version.
Klein.Buch wendet **keine** Migrationen auf Backup-Dateien an —
das würde die Integritätsgarantie des Backups brechen.

Lösung: Installiere genau die Klein.Buch-Version, mit der das
Backup erstellt wurde. Im Backup-Verlauf steht das Datum, das hilft
bei der Wahl. Spiel das Backup mit der passenden Version ein. Beim
nächsten Start der aktuellen Klein.Buch-Version migriert die App
die Datenbank automatisch auf das aktuelle Schema.

## Wiederherstellung läuft, App startet danach nicht mehr

Symptom: Nach der Wiederherstellung meldet die App
"Datenbank-Verbindung fehlgeschlagen" beim Neustart.

Wahrscheinliche Ursache: Der Vorgang wurde mitten in einem
Schreib-Schritt unterbrochen, oder die Datenbank-Datei ist auf
dem Ziel nicht ganz geschrieben (Festplatte voll).

Lösung: Klein.Buch hat vor der Wiederherstellung das
Sicherheits-Backup gezogen. Spiel dieses Backup zurück. Du landest
auf dem Stand vor dem missglückten Versuch. Anschließend versuche
die ursprüngliche Wiederherstellung erneut, diesmal mit
ausreichend Platz auf dem Ziel-Laufwerk.

## Misslungene Versuche beschädigen keine Backups

Auch wenn ein Wiederherstellungs-Versuch misslingt: Deine
Backup-Dateien bleiben unverändert. Klein.Buch öffnet sie nur
lesend. Du kannst einen Versuch beliebig oft wiederholen.

## Vorbeugen

1. Vor wichtigen Wiederherstellungen das Sicherheits-Backup
   prüfen (Klein.Buch macht das standardmäßig, der Hinweis
   steht im Dialog).
2. Backup-Datei vor dem Vorgang lokal kopieren, statt direkt aus
   einem Cloud-Ordner zu lesen. Das schließt Synchronisations-
   Konflikte aus.
3. Passphrase in einem Passwort-Manager halten.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
