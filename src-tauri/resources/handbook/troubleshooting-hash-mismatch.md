---
slug: troubleshooting-hash-mismatch
title: Hash-Mismatch im Archiv
category: troubleshooting
order: 540
keywords: [hash, mismatch, archiv, integrität, manipulation, prüfwert, sha-256]
---

# Hash-Mismatch im Archiv

Klein.Buch berechnet beim Ausstellen jeder Rechnung einen
SHA-256-Prüfwert über die Archiv-Datei und speichert ihn in der
Datenbank. Bei jedem Lese-Zugriff prüft die App, ob der aktuelle
Prüfwert noch zum gespeicherten Wert passt. Stimmen die Werte nicht
überein, schlägt Klein.Buch Alarm: ein Hash-Mismatch.

## Was das bedeutet

Eine Datei im Archiv-Ordner hat sich verändert, seit sie
ausgestellt wurde. Klein.Buch selbst tut das nicht; die App
erlaubt sich keinen schreibenden Zugriff auf ausgestellte
Archiv-Dateien. Die Veränderung muss von außen kommen.

Klein.Buch unterscheidet zwei Fälle und zeigt sie in der
Detail-Ansicht des betroffenen Belegs:

**Inhalt verändert (Manipulation).** Die Datei existiert noch,
aber ihr Inhalt passt nicht mehr zum gespeicherten Prüfwert. Das
ist der ernste Fall.

**Datei fehlt (verwaist).** Die Datei ist gar nicht mehr im
Archiv-Ordner. Das deutet auf versehentliches Löschen oder einen
Synchronisations-Fehler.

## Mögliche Ursachen

1. Ein Anti-Virus-Programm hat die Datei in Quarantäne verschoben.
2. Ein Cloud-Sync-Konflikt hat die Datei überschrieben (passiert
   bei OneDrive-Konflikten, wenn zwei Geräte denselben Ordner
   synchronisieren).
3. Manueller Eingriff (Datei verschoben, in einem Editor geöffnet
   und neu gespeichert).
4. Ein Festplatten-Fehler hat einzelne Bytes verändert.
5. Ein bösartiger Eingriff (Manipulation).

## Was Du tun musst

Es gibt in Klein.Buch keinen Knopf, der den Prüfwert neu berechnet
oder den Beleg "akzeptiert" — das wäre ein GoBD-Bruch und
absichtlich nicht eingebaut. Bei einem Hash-Mismatch ist der
einzige saubere Weg: **das Backup wiederherstellen, das vor dem
Mismatch erzeugt wurde.**

Wichtig: Eine Wiederherstellung setzt **Datenbank und gesamtes
Archiv** auf den Backup-Zeitpunkt zurück. Belege, die Du seitdem
ausgestellt hast, sind danach nicht mehr in Klein.Buch — Du musst
sie aus dem Sicherheits-Backup (das Klein.Buch vor jeder
Wiederherstellung automatisch zieht) zurückholen oder neu
erfassen.

Vorgehen:

1. **Öffne den Beleg** in der Detail-Ansicht. Klein.Buch zeigt
   eine Warnung mit dem konkreten Datei-Pfad und ob Inhalt verändert
   oder Datei verwaist ist.
2. **Sichere den aktuellen Daten-Ordner** außerhalb von Klein.Buch
   (zum Beispiel komplett kopieren), damit Du nichts unbeabsichtigt
   verlierst.
3. **Spiele das letzte Backup vor dem Mismatch zurück** (siehe
   Kapitel "Backup und Wiederherstellen").
4. **Trag die Belege seit dem Backup-Zeitpunkt erneut ein**, soweit
   sie nicht im Sicherheits-Backup vor der Wiederherstellung
   stehen.

## Auf Windows: Schreibschutz ist nicht hart

Klein.Buch markiert Archiv-Dateien beim Schreiben als nur-lesbar
(unter Windows das Read-Only-Attribut). Das schützt vor Versehen,
nicht vor vorsätzlichem Zugriff: ein Administrator kann das
Attribut im Eigenschaften-Dialog des Datei-Explorers entfernen.
Genau deshalb gibt es zusätzlich den SHA-256-Prüfwert in der
Datenbank — wenn jemand das Attribut umgeht und die Datei ändert,
fällt es beim nächsten Lese-Zugriff auf.

## Vorbeugen

1. Anti-Virus-Software so konfigurieren, dass der Klein.Buch-
   Daten-Ordner ausgespart bleibt.
2. Den Klein.Buch-Daten-Ordner **nicht** mit Cloud-Diensten
   synchronisieren. Off-Site-Sicherung läuft über die separat
   eingerichteten Backup-Ziele.
3. Regelmäßig die Archiv-Integritätsprüfung in "Einstellungen →
   Protokoll & Datensicherheit" laufen lassen, damit Du Probleme
   früh entdeckst.

## Bei wiederholten Mismatches

Wenn das Problem mehrfach auftritt, hast Du höchstwahrscheinlich
einen Cloud-Sync oder Anti-Virus, der sich am Archiv-Ordner
vergreift. Such die Ursache und stell sie ab. Andernfalls
riskierst Du eine GoBD-relevante Lücke in der Beleg-Aufzeichnung.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
