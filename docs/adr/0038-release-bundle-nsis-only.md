# ADR 0038 — Release-Bundle: NSIS only, MSI verworfen

**Status:** Akzeptiert · 2026-05-27 · v1.0-RC (Block G4.3).

**Code-Referenzen:**

- `block-g4-3: drop MSI target (calver year 2026 > 255-cap on MSI ProductVersion major); NSIS-only bundle + SHA256SUMS scoped to .exe` (`bdedb72`)
- `klein-buch/src-tauri/tauri.conf.json` — `bundle.targets = ["nsis"]`
- `.github/workflows/release.yml` — SHA256SUMS + Draft-Release-Upload nur noch für `.exe`

## Kontext

G4.1 (Block `block-g4-1-fix`, 2026-05-26) hat Klein.Buch auf **CalVer** umgestellt:
`Cargo.toml::version = "2026.5.0"` als Single Source, `tauri.conf.json::version`
entfernt (Tauri-Fallback liest die Cargo-Version). Das Schema ist
`YYYY.M.PATCH` — das Jahr ist Hauptkomponente.

G4.3 (dieser Block) hat den Release-Workflow erstmals real ausgeführt. Im
Tauri-Bundle-Step ist der MSI-Pfad mit dieser Meldung gestorben:

```
failed to bundle project `app version major number cannot be greater than 255`
```

## Problem

Der **Windows-Installer (MSI)** hat eine harte Constraint auf seine
`ProductVersion`: sie muss dem Format `MAJOR.MINOR.BUILD` mit folgenden
Werte-Bereichen entsprechen:

- `MAJOR`: 0..255 (8-Bit)
- `MINOR`: 0..255 (8-Bit)
- `BUILD`: 0..65535 (16-Bit)

Das ist seit MSI 1.0 so, dokumentiert von Microsoft und unverhandelbar — die
Windows-Installer-Engine nutzt diese Felder zur Versions-Vergleichs-Logik beim
Upgrade-Pfad.

Klein.Buchs CalVer-MAJOR ist `2026`. `2026 > 255` → MSI-Bundle bricht ab.

Tauri 2 (cargo_packager) bietet keinen offiziellen Override für die
MSI-ProductVersion getrennt von der Cargo-Version. Es gibt keinen
`bundle.windows.wix.version`-Schlüssel; die ProductVersion wird aus
`tauri::Builder` automatisch aus der Cargo-Version abgeleitet. Eine
abweichende MSI-Version würde verlangen, dass wir eigene WiX-Templates pflegen
und die Version dort hart-codieren — und dann sind Cargo-Version und
MSI-Version dauerhaft entkoppelt, was Upgrade-Logik und Doku verkompliziert.

## Entscheidung

**MSI fliegt für v1.0 raus.** Klein.Buch wird ausschließlich als
**NSIS-Installer (`.exe`)** ausgeliefert. NSIS hat keine analoge
Versions-Constraint und akzeptiert CalVer-Jahre uneingeschränkt.

Konkret:

- `tauri.conf.json::bundle.targets` von `"all"` auf `["nsis"]`.
- `.github/workflows/release.yml` `SHA-256-Checksums`-Step sammelt nur `.exe`,
  Throw-Message angepasst.
- Draft-Release-Upload-Liste ohne `bundle/msi/*.msi`.
- `docs/RELEASE-1.0-GUIDE.md` G4.3-Eintrag von „NSIS+MSI" auf „NSIS" angepasst
  mit Verweis auf dieses ADR.

## Begründung

1. **NSIS deckt den Use-Case ab.** Klein.Buch ist eine Single-User-Desktop-App
   für §19-Kleinunternehmer. NSIS installiert nach `%LOCALAPPDATA%` oder
   `Program Files`, registriert Uninstaller, schreibt Start-Menü-Einträge,
   patcht Upgrades sauber — alles, was ein End-User braucht. Tauri 2
   empfiehlt NSIS als Default-Installer.
2. **MSI ist Corporate-GPO-Deployment-Tool.** Der Mehrwert von MSI liegt in
   Active-Directory-Push-Deployment via Gruppenrichtlinie, MST-Transformen
   für Massen-Customizing und administrativen Installations-Optionen. Keine
   davon trifft auf §19-Kleinunternehmer zu — die installieren manuell vom
   Download.
3. **CalVer-Semantik bewahrt.** Das Jahr im MAJOR ist ein bewusster
   Identifikations-Anker (PRD G4.1, Decision-Log). Ihn auf `26.5.0` zu
   kappen, nur damit MSI zufrieden ist, würde die ganze CalVer-Idee
   beschädigen.
4. **Reversibel.** Wenn ein Corporate-Bedarf später aufkommt, kann MSI
   nachgeliefert werden — entweder durch CalVer-Umstellung auf `YY.M.PATCH`
   (`26.5.0`), durch ein WiX-Custom-Template mit hart-codierter MSI-Version,
   oder durch einen separaten MSI-Build-Pipeline-Branch, der die Cargo-Version
   pre-build patcht. Das ist Post-v1.0-Scope.

## Konsequenzen

**Positiv:**

- Release-Pipeline ist eine Stufe einfacher (ein Format statt zwei).
- Build-Zeit im Release-Workflow sinkt (WiX-Toolset-Download + MSI-Erzeugung
  entfallen).
- Keine Reibung mehr mit CalVer-Updates: jedes neue Jahr (2027, 2028, …)
  hätte sonst wieder MSI gekillt.
- Eine Sollbruchstelle weniger im Release-Pfad.

**Negativ:**

- Klein.Buch ist via Group Policy nicht direkt deploybar. Wer es im
  Unternehmens-Kontext ausrollen will, müsste NSIS via PowerShell-DSC oder
  Intune-Win32-App-Wrapper installieren. Realistisch nie nachgefragt.
- Wer aus Compliance-Gründen einen MSI-Installer voraussetzt (z. B. weil das
  hauseigene Software-Asset-Management nur MSI inventarisiert), kann
  Klein.Buch nicht nutzen. Auch nicht der Zielnutzer.

**Neutral:**

- SmartScreen-Warnung beim Erst-Start gilt für NSIS wie für MSI gleichermaßen,
  beides ist unsigniert. ADR-Bezug: implizit via `release-signing-guide.md`,
  nicht durch diese Entscheidung verändert.

## Alternativen erwogen

1. **CalVer auf `YY.M.PATCH` zurückstutzen** (`26.5.0` statt `2026.5.0`):
   Verworfen — die volle Jahreszahl als visueller Anker ist Teil der
   G4.1-Decision; das Stutzen wäre kosmetische Selbstbeschädigung an einer
   bewussten Konvention.
2. **Eigenes WiX-Template mit hartcodierter MSI-Version**: Verworfen —
   verlangt dauerhaftes Pflegen einer parallelen Versions-Spur (Cargo vs.
   MSI), erschwert Upgrade-Pfad-Reasoning. Aufwand-Nutzen-Verhältnis
   katastrophal für ein Format, das niemand verlangt.
3. **MSI komplett raus und v1.0 als „NSIS only" releasen, MSI für v1.1
   nachliefern**: Verworfen — niemand hat MSI angefragt, und v1.1 ist nicht
   geplant. Wäre Bürokratie für Bürokratie.

## Letzte Verifikation

2026-05-27 (G4.3-Lauf, NSIS-only Workflow grün). Dieser ADR ist ab v2026.5.0
maßgeblich; sollte er später revidiert werden (z. B. Corporate-MSI nachträglich
gewünscht), wird das per Amendment am gleichen ADR dokumentiert, nicht durch
einen neuen Eintrag.
