# ADR 0014 — Lizenz AGPL-3.0-or-later

**Status:** Akzeptiert · 2026-05-19 · Block 1. (Decision-Log D-06)

## Kontext

Klein.Buch könnte später eine gehostete Premium-Variante bekommen. Der Open-Source-
Markt für DE-Kleinunternehmer-Buchhaltung ist von proprietären SaaS dominiert; eine
echte Open-Source-Lücke existiert. Eine SaaS-Aneignung des Codes ohne Rückgabe soll
verhindert werden.

## Entscheidung

**AGPL-3.0-or-later.** Auch bei Netzwerk-Nutzung (Hosted-Variante) müssen
Änderungen offengelegt werden.

## Konsequenzen

- Schutz vor proprietärer SaaS-Aneignung; Beiträge fließen zurück.
- Mögliche Hürde für rein kommerzielle Forks (gewollt).
- Eine künftige Dual-Licensing-Option (kommerziell + AGPL) bleibt offen, da der
  Urheber die Rechte hält.

## Alternativen

| Option | Contra |
|---|---|
| MIT | Erlaubt proprietäre SaaS-Aneignung ohne Rückgabe |
| GPL-3.0 | Greift bei reinem Netzwerk-Hosting nicht (kein „Vertrieb") |
