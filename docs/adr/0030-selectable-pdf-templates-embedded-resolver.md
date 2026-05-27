# ADR 0030 — Wählbare PDF-Vorlagen: eingebettete Unified-Built-ins + inputs-Override + Sentinel-Auflösung

**Status:** Akzeptiert · 2026-05-22 · Block 17a. Keine Migration.

## Kontext

Ziel G30: mindestens **3 wählbare PDF-Vorlagen** (Modern/Klassisch/Minimal),
und zwar **für Rechnung UND Angebot** (Manuel-Entscheidung). Spannung mit der
Hard-Rule **`inputs/` ist für Maschinen tabu**: mitgelieferte Vorlagen können
nicht als `inputs/`-Dateien ausgeliefert/aktualisiert werden. Zusätzlich
unterscheiden sich die Daten-Schemata von Rechnung (`data.invoice`) und Angebot
(`data.quote`), und der Name `default` kollidiert (das Rechnungs-Default-Template
`inputs/pdf-templates/default.typ` taugt nicht fürs Angebot).

## Entscheidung

1. **3 eingebettete Unified-Templates** als Binary-Konstanten
   (`pdf::templates::TEMPLATE_{MODERN,KLASSISCH,MINIMAL}`). **Eine** `.typ`
   rendert Rechnung **oder** Angebot via internem Branching
   (`data.at("invoice", …)`). Differenzierung über **Layout/Farbe/Gewicht**, nicht
   die Fontfamilie — das `typst-assets`-Bundle hat **keinen Sans** (Libertinus
   Serif / New Computer Modern); „Modern" nutzt den Petrol-Akzent.
2. **Doctype-bewusster Resolver:** `inputs/{name}.typ`-Override gewinnt → sonst
   eingebettetes Built-in. **`default` bleibt das Legacy-Paar** (Rechnung =
   `inputs/default.typ`, Angebot = `DEFAULT_QUOTE_TEMPLATE`) mit
   **Kollisions-Schutz**: ein Angebot mit `default` lädt **nie** `default.typ`.
3. **Globale Auswahl** über `seller_profile.default_pdf_template`. **Sentinel:**
   ist der Beleg-Wert `pdf_template = 'default'`, wird beim Render der globale
   Default aufgelöst. Das **archivierte PDF ist der GoBD-Snapshot** — der
   Beleg-Feldwert bleibt `'default'` (kein nachträgliches UPDATE auf
   festgeschriebene Belege, das der Immutability-Trigger ohnehin ablehnen würde).
4. **§19-Enforcement:** jede Built-in trägt den Marker
   `// §19-KLAUSEL-BLOCK: REQUIRED` und rendert `kleinunternehmer.hinweis_text`
   (besteht `pdf::klausel_check`); der Switcher **sperrt nicht-§19-konforme**
   Vorlagen, solange Kleinunternehmer aktiv ist.
5. **Logo** über `World::file()` (Typst kann kein base64) am rooted Pfad
   `/branding/logo.<ext>`. **Vorschau-PDF** je Vorlage mit Dummy-Daten
   („Max Mustermann" + geometrisches Dummy-Logo).

## Konsequenzen

- Mitgelieferte Vorlagen ohne `inputs/`-Schreibzugriff — die Hard-Rule bleibt
  intakt; der Nutzer kann jede Vorlage per `inputs/{name}.typ` überschreiben.
- Globale Auswahl wirkt auf neu gerenderte Belege; bereits ausgestellte bleiben
  unverändert (ihr archiviertes PDF ist fix).
- Eine Vorlage pro Stil (statt je ein Rechnungs- + ein Angebots-File) → weniger
  Duplikation, dafür internes Branching im Template.

## Alternativen

| Option | Contra |
|---|---|
| Je separate Rechnungs- + Angebots-Datei pro Stil (6 Dateien) | doppelte Pflege; das Branching in einer Datei ist überschaubar |
| Vorlagen als `inputs/`-Dateien ausliefern | verletzt die `inputs/`-Tabu-Hard-Rule; kein Update via Binary möglich |
| Auswahl pro Beleg im UI | Feld (`invoices/quotes.pdf_template`) existiert, aber mehr UI-Fläche — auf später vertagt; global deckt 95 % |

## Referenzen

`pdf::templates` (`TEMPLATE_*`, `resolve_invoice_template`,
`resolve_quote_template`, `list_templates`), `commands::settings`
(`pdf_templates_list`, `seller_default_template_set`, `pdf_template_preview`),
`commands::invoices::run_lock_pipeline` + `commands::quotes` (Sentinel-Render),
`pdf::klausel_check`; ADR 0008 (Typst/Mustang), §19-Hardline (CLAUDE.md).
Commit `block-17a`.
