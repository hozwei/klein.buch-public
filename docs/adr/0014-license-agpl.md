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

## Amendment 2026-10-23 — Web-/Docker-Betrieb (ADR 0039)

**Status:** Akzeptiert · 2026-10-23 · Phase W (ADR 0039, E13; Blöcke W1, W12, W13).
Klein.Buch wird als öffentliches Container-Image verteilt und über das Netz genutzt; damit
greift AGPL §13, und das Image enthält Fremdteile (OpenJDK, Debian-Pakete, JARs) mit eigenen
Lizenzpflichten. Festgelegt wird:

- **`LICENSE`** = AGPL-3.0-Volltext (byte-genau, CI prüft die Prüfsumme); Kopf in `COPYRIGHT`.
- **Lizenzseite im Image** unter `/legal/` (ohne Anmeldung) und `/usr/share/doc/klein-buch`:
  Lizenzen und Hinweise der Rust-, JS- und Java-Komponenten sowie die Debian-Paketliste.
- **Quellen Dritter:** `SOURCES.md` im Image und je Release das Asset
  `klein-buch-<Version>-quellen-dritter.tar.zst`; ohne Quellarchiv kein Image-Push.
- **OCI-Labels** `org.opencontainers.image.source`, `.revision`, `.version` und `.licenses`.
- **§13-Quelllink konfigurierbar:** `GET /api/public/info` liefert Version, Commit, Quell-URL
  und SPDX-Kennung. Betreiber mit eigenen Änderungen setzen `KLEIN_BUCH_SOURCE_URL` auf ihren
  Quellstand.
- **Tags `v*` und Images werden nie gelöscht oder verschoben:** Sie sind der Quellbezug
  älterer Images und der Rückfall nach einem gescheiterten Update (ADR 0039, E15).
- **Beiträge Dritter** werden vorerst nicht angenommen (`CONTRIBUTING.md`), bis der Urheber als
  Rechteinhaber über CLA bzw. Rechteübertragung entschieden hat. Nur so bleibt die
  Dual-Licensing-Option (Konsequenzen oben) offen.
