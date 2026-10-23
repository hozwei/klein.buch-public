# ADR 0013 — Local-First strict: keine Telemetrie, kein Auto-Update, kein Cloud-Sync

**Status:** Akzeptiert · 2026-05-19 · Block 1. (Decision-Log D-26)

## Kontext

Es geht um Steuer- und Kundendaten Dritter (DSGVO-relevant). Vertrauen entsteht aus
Datensparsamkeit und Nachvollziehbarkeit, nicht aus Cloud-Komfort.

## Entscheidung

- **Keine Telemetrie**, **kein Auto-Update**, **kein Cloud-Sync** im Produkt.
- Ausgehender Netzwerkverkehr nur bei **explizitem** User-Trigger (Versand) und
  ausschließlich gegen den Sidecar (KoSIT/Mustang lokal) bzw. den vom Nutzer
  konfigurierten SMTP-Server.
- Backups liegen lokal/in einem vom Nutzer gewählten Ordner (z. B. OneDrive),
  immer verschlüsselt (ADR 0009).

## Konsequenzen

- Keine versteckten Datenabflüsse; einfacher DSGVO-Stand für den Single-User.
- Updates und Multi-Device-Sync sind manuell bzw. spätere Architekturthemen
  (Hosted-Variante erst ab v0.3, separat zu bewerten).
- Kein Crash-/Usage-Reporting — Fehlerdiagnose erfolgt über lokale Logs.

## Referenzen

CLAUDE.md „Local-First strict"; PRD §4.

## Amendment 2026-10-23 — Web-/Docker-Betrieb (ADR 0039)

**Status:** Akzeptiert · 2026-10-23 · Phase W (ADR 0039, E1/E14; Umsetzung Blöcke W5, W12).
Klein.Buch läuft als Server im Container, der Nutzer arbeitet im Browser. Das ändert den
Netzwerk-Teil dieser Entscheidung, nicht ihren Kern:

- **Eingehend:** ein HTTP-Listener für den Browser-Zugriff, vorgesehen hinter einem
  Reverse-Proxy mit TLS. Ohne Sitzung erreichbar sind nur Einrichtung/Anmeldung,
  `GET /api/public/info`, `/healthz` und `/legal/` (ADR 0039).
- **Weiterhin keine Telemetrie, kein Auto-Update, kein Cloud-Sync**, kein
  Crash-/Usage-Reporting.
- **Neu, nur opt-in:** eine Versionsprüfung gegen die GitHub-Releases des öffentlichen
  Repositorys. Außer der Abfrage selbst werden keine Daten gesendet; Name und Link einer neuen
  Version erscheinen als Text (nie HTML) in der Inbox. Ohne Opt-in keine Verbindung.
- **Das Update löst der Betreiber aus**, in seiner Container-Verwaltung. Die App tauscht ihr
  Image nicht selbst: Dafür bräuchte sie `docker.sock`, also praktisch root auf dem Host, was
  für einen aus dem Internet erreichbaren Dienst ausgeschlossen ist (ADR 0039, E14).
- **Ausgehend** nur: Versand (SMTP), Off-Site-Backup (SFTP, ADR 0034) und die opt-in
  Versionsprüfung. Der Sidecar läuft lokal im Container.
