# ADR 0008 — PDF via Typst, ZUGFeRD via Mustang im Java-Sidecar

**Status:** Akzeptiert · 2026-05-19 · Block 3. (Decision-Log D-09, D-10)

## Kontext

Rechnungen brauchen ein menschenlesbares PDF und — für ZUGFeRD — ein PDF/A-3 mit
eingebettetem CII-XML. Die PDF-Erzeugung soll Rust-nah und mit human-editierbaren
Templates erfolgen; die ZUGFeRD-Einbettung muss robust und standardkonform sein.

## Entscheidung

- **Typst (als Library)** erzeugt das Layout-PDF (PDF/A-3b) aus
  `inputs/pdf-templates/*.typ`; Daten kommen als `json.decode(sys.inputs…)`.
- **Mustang Project** (Apache-2.0) erzeugt aus dem PDF/A + CII das ZUGFeRD-PDF/A-3.
  Mustang läuft mit dem KoSIT-Validator (ADR 0007) gemeinsam in **einem
  jlink-JRE-Sidecar** (Ein-Schritt-`combine`).

## Konsequenzen

- Templates sind versionierbar und ohne Code-Änderung anpassbar; §19-Marker-Check
  (ADR 0005) erzwingt Konformität.
- Ein einziger Java-Sidecar deckt Validierung + ZUGFeRD ab (kleinere Wartung).
- Sidecar ist plattformspezifisch (jlink) → Cross-OS-Bundles per CI-Matrix
  (ADR 0001).
- Pipeline-Reihenfolge: Typst PDF/A-3b zuerst (Mustang verlangt die PDF/A-Kennung).

## Alternativen

| Option | Contra |
|---|---|
| ZUGFeRD-Einbettung selbst (lopdf) | PDF/A-3-Spezifika fehleranfällig, hoher Aufwand |
| wkhtmltopdf/Chromium-Print | Kein PDF/A, großes Binary |
| Mustang als Rust-Reimpl | ZUGFeRD-Spec non-trivial, Out-of-Scope v0.1 |
