# ADR 0001 — Cross-OS-Sidecar-Bundles in Block 17 statt Block 0

**Status:** Akzeptiert · 2026-05-19 · Block 0.

## Kontext

PRD §5 verlangt in Block 0 jlink-Bundles für vier Target-Triples:
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

Das Java-Sidecar bündelt KoSIT-Validator und Mustang in einer
jlink-Minimal-JRE. `jlink` kann jedoch nur native Images für die
**Host-Plattform** erzeugen, von der das JDK stammt. Cross-OS-Bundles
erfordern entweder:

- Target-spezifische JDKs lokal vorhalten (3× ~200 MB Downloads,
  Pflege-Aufwand), oder
- einen CI-Matrix-Build auf nativen Runnern.

## Entscheidung

**In Block 0 wird nur das Windows-x86_64-Bundle lokal gebaut.** Das ist
Manuels Dev-Plattform (Solo-Dev, Windows 11). Die drei anderen Triples
werden in **Block 17** (Release-Workflow) über eine GitHub-Actions-Matrix
mit den nativen Runnern erzeugt:

| Triple | GitHub-Actions-Runner |
|---|---|
| `x86_64-pc-windows-msvc` | `windows-latest` |
| `x86_64-apple-darwin` | `macos-13` |
| `aarch64-apple-darwin` | `macos-14` |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` |

Build-Script: `scripts/build-sidecar.ps1` (für Windows lokal) wird in
Block 17 portiert nach Plattform-spezifische Scripts (`build-sidecar.sh`).

## Konsequenzen

- **Block 0 bleibt für einen Solo-Dev tragbar** — kein Target-JDK-Management.
- **Lokales Build/Test funktioniert nur auf Windows bis Block 17.** Manuel
  kann keine Cross-OS-Tests vor Block 17 fahren.
- **Release v0.1.0** (Tag von Block 17) ist der erste Punkt, an dem es
  offizielle Bundles für alle vier Plattformen gibt.
- **`tauri.conf.json`** ist bereits so konfiguriert, dass `bundle.resources`
  mit `binaries/**` alle Sidecar-Triples mitnimmt, sobald sie da sind.
- **`config.rs`** kennt alle vier Target-Triples in `sidecar_target_triple()`
  und löst sie zur Laufzeit auf — andere Plattformen werden in einen
  klaren Fehler laufen, bis ihr Bundle existiert.

## Alternativen abgewogen

| Option | Pro | Contra |
|---|---|---|
| Vier Target-JDKs lokal + Cross-jlink | Block 0 erfüllt PRD-Wortlaut | 600+ MB Setup, fragile Cross-Tooling, kein Test-Mehrwert für Solo-Dev |
| Mustang+KoSIT nicht bundlen, Runtime-Download | Klein.Buch-Binary kleiner | Verletzt local-first-Prinzip, Erst-Start braucht Netzwerk |
| Mustang als Rust-Re-Implementation | Eliminiert Java-Dependency | ZUGFeRD-Spec non-trivial, hoher Wartungsaufwand, Out-of-Scope für v0.1.0 |
| **Block 17 via CI-Matrix** (gewählt) | Saubere, reproduzierbare Builds; native Runner; bis dahin kein Schaden | v0.1.0 ist der erste Cross-OS-Release |

## Referenzen

- jlink-Doku: https://docs.oracle.com/en/java/javase/21/docs/specs/man/jlink.html
- GitHub-Actions-Runner-Images: https://github.com/actions/runner-images
- Block-0-Build-Notes: `~/cowork/Buchhaltung/memory/klein-buch/block-0-notes.md`
- PRD §5.5 Build-Plan / §7.5 Phase 2D Block 17
