# In-App-Handbuch + Kontext-Hilfe-Anker

> Wie das User-Handbuch im Frontend gerendert wird und wie einzelne
> UI-Stellen per `<HelpAnchor>` auf konkrete Kapitel deeplinken. Ton-
> und Inhalts-Regeln für die Handbuch-Texte selbst stehen unter
> G2-DOC.2 in `docs/RELEASE-1.0-GUIDE.md`, nicht hier — dieses Dokument
> ist nur die technische Schiene.

---

## 1. Quelle und Index

- Alle Handbuch-Seiten liegen als reine Markdown-Dateien unter
  `klein-buch/src-tauri/resources/handbook/*.md` und werden mit dem
  Tauri-Bundle ausgeliefert. Das Verzeichnis enthält zusätzlich
  `README.md` (Konventionen, kein Beleg-Artefakt). Das Handbuch ist
  bewusst **reine Textform** (G2-DOC.2.8): kein `img/`-Verzeichnis,
  keine Screenshots — die UI wird in Worten beschrieben, damit
  Bilder bei UI-Änderungen nicht veralten. Der Markdown-Renderer
  trägt einen harmlosen Bild-Fallback (broken-img-Stub) für den Fall,
  dass künftig wieder Bilder eingezogen werden.
- Jede Seite trägt einen YAML-Front-Matter mit den fünf Pflichtfeldern
  `slug`, `title`, `category`, `order`, `keywords`. Konvention:
  **Dateiname == `<slug>.md`**, der `slug` ist im Verzeichnis eindeutig
  und kebab-case. Der Test `handbook_resources_test` setzt das beim
  Build durch.
- Der Frontend-Index lebt in `klein-buch/src/lib/handbook/index.ts`:
  `import.meta.glob` zieht alle Files zur Build-Zeit, ein eigener
  schlanker Front-Matter-Parser füllt die `HandbookEntry[]`-Liste,
  sortiert sie nach Kategorie + `order` + Titel und stellt sie über
  `listEntries()`, `getEntry(slug)`, `listByCategory()`, `listSlugs()`
  bereit. Keine JS-Dependency (kein `gray-matter` o. ä.).

## 2. Routen

- `/help` → Redirect auf `willkommen`.
- `/help/<slug>` → einzelne Handbuch-Seite. Der Loader
  (`src/routes/help/[slug]/+page.ts`) holt den Eintrag aus dem Index,
  rendert den Markdown-Body mit `renderHandbookMarkdown` (`marked` v15,
  in `src/lib/handbook/render.ts`) und produziert HTML + Table-of-Contents
  aus den `h2`/`h3`-Headings. FAQ-Seiten (`category: faq`) werden von
  `transformAsFaq` in ein `<details>`-Accordion umgesetzt — die TOC-IDs
  bekommen dort das Präfix `faq-` für Deeplinks aus dem Glossar.
- Suche, Sidebar-Kapitelbaum, FAQ-Akkordeon, F1-Shortcut und der
  Druck-Stylesheet sitzen im `/help`-Layout. Details: G2-DOC.3.3 / .3.4
  in `docs/RELEASE-1.0-GUIDE.md`.

## 3. Heading-Slugs (tiefe Sprünge)

`renderHandbookMarkdown` setzt auf jede `h2`/`h3` eine deterministische
ID, die mit `slugifyHeading(text)` aus `src/lib/handbook/render.ts`
gebildet wird:

- Lowercase
- `ä → ae`, `ö → oe`, `ü → ue`, `ß → ss`
- Alle übrigen Nicht-Wort-Zeichen → Wortgrenze
- Zusammenziehung mit `-`
- Konflikte mit `-1`, `-2`, … aufgelöst (in Render-Reihenfolge)

Wer aus dem Code auf eine konkrete Überschrift deeplinken will, ruft
denselben Slugifier auf — die `<HelpAnchor>`-Komponente macht das
automatisch (siehe §4).

## 4. `<HelpAnchor>`-Komponente

`klein-buch/src/lib/HelpAnchor.svelte` — kleiner `?`-Icon-Button neben
einer Überschrift oder einem Pflicht-Button. Klick öffnet via SvelteKit-
Router das zugehörige Handbuch-Kapitel.

```svelte
<!-- Sprung auf die Kapitel-Hauptseite. -->
<HelpAnchor slug="euer-cash-basis" />

<!-- Sprung auf einen konkreten Abschnitt. heading == sichtbarer
     Heading-Text, der Slugifier formt daraus den Hash. -->
<HelpAnchor slug="paragraph-19-grundlagen" heading="Klausel" />

<!-- Tooltip-Override. Ohne label wird der Titel aus dem Handbuch-Index
     genommen, mit Fallback auf den Slug. -->
<HelpAnchor slug="backup-und-wiederherstellen" label="Backups & Restore" />
```

Props (alle pflicht in Sigantur, nur `slug` erforderlich zur Laufzeit):

| Prop      | Typ      | Bedeutung                                                         |
|-----------|----------|-------------------------------------------------------------------|
| `slug`    | `string` | Front-Matter-`slug` einer Handbuch-Seite. **Pflicht.**            |
| `heading` | `string` | Optional. Sichtbarer Heading-Text; wird zu `#…` auf der Ziel-Seite.|
| `label`   | `string` | Optional. Überschreibt den Tooltip-Text (`title=` und `aria-label`).|

Implementierungs-Notizen:

- Icon ist Inline-SVG (`<circle> + <path> + <circle>`). Keine neue
  JS-Dependency, kein Icon-Font, kein Netz-Aufruf.
- Tooltip-Text wird aus `getEntry(slug)?.title` gezogen — fällt das
  zur Laufzeit doch mal aus (Eintrag entfernt, Build nicht erneuert),
  bleibt der Slug als Fallback sichtbar. Der Verify-Test in §5
  unterbindet diesen Fall zur Build-Zeit.
- Visuelles: token-basiert (`--c-primary-600` Standard, `--c-primary-50`
  Hover, `--r-pill` Form). 22 × 22 px, daneben 6 px `margin-left`. Passt
  inline neben Heading-Text oder Button.
- `aria-label` ist gesetzt und enthält den Tooltip-Text, damit Screen
  Reader die Ziel-Seite ansagen.

## 5. Build-Time-Verify-Test

`klein-buch/src-tauri/tests/handbook_anchors_test.rs` (G2-DOC.4-A).

- Walkt rekursiv `klein-buch/src/**/*.svelte`.
- Greift mit einer Regex über die Datei-Inhalte und sammelt jeden
  Slug-Wert, den ein `<HelpAnchor slug="…">` (oder `slug='…'`) nennt
  — line-breaks im Tag-Body inklusive.
- Listet parallel den Handbuch-Index aus
  `src-tauri/resources/handbook/*.md` (Dateiname == Slug, der
  Front-Matter-Test garantiert das).
- Bricht den Build, sobald Code auf einen Slug zeigt, den es im
  Handbuch nicht gibt.

Wirkung: falsch geschriebener oder gelöschter Slug fällt in der CI auf,
nicht erst im laufenden Programm. Pendant zu `handbook_resources_test`
(prüft die Markdown-Seite ist gültig) — gemeinsam halten sie beide
Seiten der Konvention konsistent.

## 6. Platzierung an UI-Stellen (G2-DOC.4-B)

Pro Stelle: ein Import, ein `<HelpAnchor slug="…" />` neben Überschrift
oder Pflicht-Button. Slug-Zielliste steht in
`docs/RELEASE-1.0-GUIDE.md` § G2-DOC.4-B; die jeweils final gewählten
Slugs werden gegen den dann existierenden Handbuch-Index gematcht —
falsche Werte fängt der Verify-Test aus §5.
