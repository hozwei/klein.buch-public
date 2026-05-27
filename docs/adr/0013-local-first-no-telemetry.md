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
