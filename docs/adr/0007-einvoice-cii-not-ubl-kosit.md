# ADR 0007 — E-Rechnung als UN/CEFACT CII; KoSIT als Validator

**Status:** Akzeptiert · 2026-05-19 · Block 3a/3c. (Decision-Log D-05)

## Kontext

EN 16931 erlaubt zwei Syntaxen: UBL und UN/CEFACT CII. Block 3a hatte zunächst
UBL gewählt. Beim realen Sidecar-Lauf zeigte sich: Mustang/ZUGFeRD bettet
**ausschließlich CII** ein (`CustomXMLProvider` verlangt `rsm:CrossIndustryInvoice`).
Validierung muss gegen den offiziellen Standard erfolgen, nicht gegen Eigenbau.

## Entscheidung

- **XRechnung-Generierung als UN/CEFACT CII.** Element-Reihenfolge nach der
  KoSIT-Schematron-Testinstanz modelliert. §19 als BT-22-Note + BT-120
  ExemptionReason + CategoryCode `E`. Storno mit Type-Code 384 + BillingReference.
- **KoSIT-Validator** (offizielles Tool) als Java-Sidecar; muss `Passed`/`Warning`
  liefern, sonst kein PDF.

## Konsequenzen

- ZUGFeRD-Einbettung (ADR 0008) funktioniert ohne Format-Konvertierung.
- Validierung ist autoritativ (offizielle Schematron-Regeln), nicht selbstgebaut.
- Lehre (siehe Memory `verify-dont-guess`): externe Format-/CLI-Annahmen vor dem
  Code gegen die echte Quelle prüfen.

## Alternativen

| Option | Contra |
|---|---|
| UBL behalten | Mustang bettet nur CII ein → zweiter Konvertierungsschritt |
| Eigener Validator | Schematron-Regelwerk umfangreich, nicht autoritativ |

## Referenzen

EN 16931, KoSIT-Validator + XRechnung-Schematron, Mustang `ZUGFeRDExporterFromPDFA`.
