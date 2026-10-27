# ADR 0008 — PDF via Typst, ZUGFeRD via Mustang im Java-Sidecar

**Status:** Akzeptiert · 2026-05-19 · Block 3. (Decision-Log D-09, D-10)

## Kontext

Rechnungen brauchen ein menschenlesbares PDF und — für ZUGFeRD — ein PDF/A-3 mit
eingebettetem CII-XML. Die PDF-Erzeugung soll Rust-nah und mit human-editierbaren
Templates erfolgen; die ZUGFeRD-Einbettung muss robust und standardkonform sein.

## Entscheidung

- **Typst (als Library)** erzeugt das Layout-PDF (PDF/A-3b) aus
  `inputs/pdf-templates/*.typ`; Daten kommen als `json(bytes(sys.inputs…))`.
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

## Amendment 2026-10-23 — Web-/Docker-Betrieb (ADR 0039)

**Status:** Akzeptiert · 2026-10-23 · Phase W (ADR 0039; Blöcke W1, W2, W11). Typst und
Mustang bleiben gesetzt. Für den Serverbetrieb gilt zusätzlich:

- **Sidecar nur noch Linux** (Amendment ADR 0001): jlink-JRE im Image, Launcher
  `klein-buch-java.sh`. Die Konsequenz „Cross-OS-Bundles per CI-Matrix“ und das
  Windows-Sidecar entfallen. Bezugsquellen und SHA-256 der JARs/ZIPs stehen in
  `sidecar/versions.json`; ein CI-Schritt vergleicht sie mit den Originalen.
- **Last begrenzt:** Der Typst-Render läuft außerhalb der async-Runtime (`spawn_blocking`)
  hinter einer Render-Semaphore, JVM-Starts (KoSIT, Mustang) sind ebenfalls per Semaphore
  begrenzt (`limits.rs`: Render 2, JVM 2). Ein laufender Render blockiert keine anderen
  Anfragen; parallele Anfragen können keine unbegrenzte Zahl von JVMs starten.
- **Escaping vollständig:** Freitext aus dem Paket-Markup (ADR 0031) wird vor der Übergabe an
  Typst an jedem ASCII-Satzzeichen mit `\` maskiert. Vorher fehlten u. a. `/`, `=`, `~`, `-`
  und `+`; `//` begann einen Typst-Kommentar, der Text danach fehlte im PDF, stand aber im
  XML (BT-154). PDF und XRechnung zeigen damit denselben Text.

## Amendment 2026-10-25 — ZUGFeRD-Profil XRECHNUNG (Lauf F5c, R3-017)

Das Rechnungs-PDF wird mit dem Mustang-Profil `x` (**XRECHNUNG**) erzeugt statt `e`
(EN 16931); Konstante `einvoice::mustang_bridge::ZUGFERD_PROFILE`. Grund: Das
eingebettete XML ist eine XRechnung 3.0 (CII) mit der Guideline-ID
`urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0` (BT-24).
ZUGFeRD 2.x (seit 2.1.1) ordnet dieser Guideline das Profil XRECHNUNG zu — XMP
`fx:ConformanceLevel` = `XRECHNUNG`, Anhang `xrechnung.xml`; das Profil EN 16931
gehört zur Guideline `urn:cen.eu:en16931:2017` (Anhang `zugferd-invoice.xml`).
Bisher widersprachen sich Container („EN 16931“) und XML (XRechnung). Beide Profile
sind EN-16931-konform und damit E-Rechnung nach § 14 Abs. 1 UStG (BMF-Schreiben vom
15.10.2024; ausgeschlossen sind nur MINIMUM und BASIC-WL); der Mustang-Validator prüft
beide Fassungen als gültig (PDF/A-3u und XML, 25.10.2026 an derselben Rechnung
verglichen). Das XML selbst ist unverändert (KoSIT wie bisher). Der eigene Empfang
akzeptiert XRECHNUNG (ADR 0037). Test: `sidecar_real_mustang_combine_and_extract_pdfa3`
(XMP `XRECHNUNG`, `xrechnung.xml`, Extraktion bitgleich). Alternative „bewusst bei
EN 16931 bleiben“ verworfen, weil sie den Widerspruch zum XML stehen ließe; ein
Wechsel der Guideline auf reines EN 16931 schiede für B2G (XRechnung-Pflicht) aus.

## Amendment 2026-10-25 — Typst 0.15 (Lauf F6b, LE-TYPST)

Typst läuft ab 2026.11 in Fassung **0.15** (vorher 0.13, über 0.14 als Zwischenstufe
geprüft; Begründung und Messwerte in ADR 0039 Nr. 139). Für Vorlagen gilt:

- Daten liest eine Vorlage mit `json(bytes(sys.inputs.at("data-json")))`. `json.decode`
  ist in Typst 0.15 entfernt; alle eingebauten Vorlagen, `inputs/pdf-templates/default.typ`
  und die Testvorlagen sind umgestellt. Eine ältere eigene Vorlage bricht mit einer
  Meldung ab, die den Ersatz nennt (`typst_render::with_upgrade_hint`). Neu ist ein Test,
  der die eingebauten Vorlagen auf Typst-Warnungen prüft (`pdf::snapshot::
  builtin_templates_compile_without_typst_warnings`): Veraltetes fällt so vor der Entfernung
  auf.
- `PagedDocument` kommt aus `typst-layout`; Dateien der Vorlage heißen intern
  `RootedPath(Project, /main.typ | /branding/…)`.
- Das PDF trägt ein Erstellungsdatum (UTC, über `crate::clock`); PDF/A-3b verlangt es seit
  Typst 0.14. Typst schreibt seit 0.14 ein **getaggtes PDF** (Struktur für Screenreader); das
  Rechnungs-PDF wird dadurch etwa ein Drittel größer (Beispiel 29 → 40 KB), das Bild bleibt
  gleich. Mustang nimmt das PDF/A-3b aller vier Vorlagen an
  (`sidecar_real_mustang_accepts_every_builtin_template`).
- Vor jedem weiteren Typst-Update: Referenz-PDFs mit `pdf::snapshot` vorher und nachher
  erzeugen (`KB_PDF_SNAPSHOT_DIR=… cargo test --lib -- --ignored pdf_snapshot_`) und Text
  sowie Pixel vergleichen; übernommen wird nur bei gleichem Text und unsichtbaren
  Pixel-Abweichungen (Schwelle in ADR 0039 Nr. 139).
