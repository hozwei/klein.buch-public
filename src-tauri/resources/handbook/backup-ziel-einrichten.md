---
slug: backup-ziel-einrichten
title: Off-Site-Backup einrichten
category: erste-schritte
order: 40
keywords: [backup, off-site, onedrive, sftp, cloud, sicherung]
---

# Off-Site-Backup einrichten

Klein.Buch schreibt nach jeder Festschreibung und einmal pro Tag
automatisch ein verschlüsseltes Backup. Eine **lokale Sicherung**
landet immer im Klein.Buch-Daten-Ordner und ist nicht
konfigurierbar — die richtest Du nicht ein. Zusätzlich kannst Du
ein **Off-Site-Ziel** angeben (ein Ordner irgendwo außerhalb des
Daten-Ordners), damit die Backups einen Hardware-Schaden,
Diebstahl oder Wohnungsbrand überleben.

## Welche Off-Site-Ziele möglich sind

**Verzeichnis.** Jeder Ordner, der auf Deinem Rechner als normaler
Datei-Explorer-Ordner sichtbar ist. Das umfasst USB-Sticks, externe
Festplatten, Netzlaufwerke im Heimnetz und vor allem
Cloud-Sync-Ordner (OneDrive, iCloud, Dropbox, Nextcloud — Klein.Buch
schreibt in den Ordner, der Cloud-Sync-Client lädt die Datei dann
selbst hoch).

**SFTP.** Ein SFTP-Server (SSH File Transfer Protocol, sichere
Datei-Übertragung über SSH) im Internet. Sinnvoll, wenn Du einen
eigenen Server betreibst.

Du kannst beide Varianten parallel haben. Wenn Du keines einrichtest,
läuft nur die lokale Sicherung — verlässt Du im Schadensfall den
Rechner, sind die Daten weg.

## Cloud-Ordner einrichten

Stelle vorher sicher, dass der Cloud-Dienst (OneDrive, iCloud,
Dropbox, Nextcloud) auf Deinem Rechner installiert und angemeldet
ist und mindestens einen Ordner mit Deinem Online-Konto
synchronisiert. Du erkennst das daran, dass der Ordner als normaler
Datei-Explorer-Ordner sichtbar ist.

Geh in Klein.Buch auf "Einstellungen" und "Backups". In der Karte
**"Off-Site-Kopie"** trägst Du den Pfad ein oder wählst ihn über
**"Durchsuchen…"**. Klein.Buch erkennt typische Cloud-Ordner
automatisch und schlägt einen Pfad vor. Klick auf "Speichern".

Klein.Buch erzeugt mit dem nächsten Backup-Lauf automatisch eine
Kopie im neuen Ziel; Du kannst auch sofort über **"Jetzt sichern"**
ein Backup auslösen und das Ergebnis im Backup-Protokoll prüfen.

## SFTP-Ziel einrichten

Du brauchst die Anmelde-Daten Deines SFTP-Anbieters: Server-Adresse
(zum Beispiel `backup.beispiel-server.de`), Port (meistens 22),
Benutzername, Ziel-Ordner auf dem Server und Passwort.

Scroll auf der Backups-Seite zur Karte **"SFTP-Server (für
Fortgeschrittene)"** und trage die Anmelde-Daten ein. Klick auf
**"Verbindung testen"**. Beim ersten Verbinden zeigt Klein.Buch Dir
den Server-Schlüssel-Fingerabdruck im Format `SHA256:<base64>` an.
Vergleiche ihn mit dem Wert, den der Server-Anbieter Dir vorab
mitgeteilt hat. Stimmt er, klick auf **"Als SFTP-Ziel speichern"**
— Klein.Buch merkt sich den Fingerabdruck.

Wenn Du den Server selbst betreibst, fragst Du den Fingerabdruck
auf dem Server-Terminal ab:

```
ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
```

Klein.Buch speichert das SFTP-Passwort im Schlüsselbund Deines
Betriebssystems (Windows Credential Manager), nicht in der App
selbst. Es taucht nirgends im Klartext auf.

## Erstes Backup prüfen

Lös über "Jetzt sichern" ein Backup aus oder warte auf das nächste
automatische Backup. Öffne dann auf der Backups-Seite oben rechts
das **"Backup-Protokoll"**. Du siehst Datum, Größe, Ort und Status
für jedes erzeugte Backup. Steht ein Eintrag auf "Fehler", klick
darauf — Klein.Buch zeigt die genaue Fehlermeldung.

## Cloud-Anbieter-Stolperstein

OneDrive und iCloud halten Dateien manchmal nicht lokal vor,
sondern nur als Platzhalter ("Files On Demand", "Optimize Mac
Storage"). Das ist für Backups eine schlechte Idee, weil die Datei
beim nächsten Zugriff erst aus der Cloud nachgeladen werden muss.
Setze für Deinen Klein.Buch-Backup-Ordner die Einstellung des
Cloud-Dienstes auf "Immer auf diesem Gerät behalten".

## Wenn ein USB-Stick als Off-Site-Ziel dient

Klein.Buch schreibt nur dann auf den Stick, wenn er beim
Backup-Zeitpunkt angesteckt ist. Vergisst Du den Stick, gibt es
einen Fehler-Eintrag im Backup-Protokoll und einen Hinweis unter
"Hinweise". Die lokale Sicherung läuft währenddessen weiter.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
