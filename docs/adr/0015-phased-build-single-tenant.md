# ADR 0015 — Phasen-Build (18 Blöcke / 5 Phasen), Single-Tenant strict

**Status:** Akzeptiert · 2026-05-19 · Block 1. (Decision-Log D-27, D-07)

## Kontext

Ein Solo-Entwickler baut ein compliance-kritisches Produkt. Risiko und früher
Nutzwert müssen balanciert werden. Multi-Tenant-Fähigkeit wäre ein tiefer
Schema-Eingriff, der jetzt keinen Nutzen bringt.

## Entscheidung

- **Phasen-Build: 18 Blöcke in 5 Phasen.** Phase 1 (0–5) liefert ein produktiv
  nutzbares Werkzeug (Kontakte, Rechnung+ZUGFeRD+Versand, Storno, Archiv, Backup);
  Phasen 2A–2D ergänzen Angebote, Kosten, E-Rechnung-Empfang, Anlagen/AfA/EÜR,
  Notifications/OAuth/Polish.
- **Block-by-Block-Disziplin**: ein Block, Done-Checks, Commit, Go abwarten.
- **Single-Tenant strict** in v0.1; Multi-Tenant ist ein bewusster v0.3-Schema-Bruch.

## Konsequenzen

- Früher, echter Nutzwert nach Phase 1; Risiko inkrementell.
- Schema ist auf einen Mandanten optimiert (kein `tenant_id`-Overhead).
- Multi-Tenant erfordert später Migration + Architektur-Review (nicht rückwärts-
  kompatibel) — explizit akzeptiert.

## Referenzen

PRD §7 Build-Plan; CLAUDE.md Block-by-Block-Regeln.
