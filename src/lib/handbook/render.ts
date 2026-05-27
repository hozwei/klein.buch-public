// G2-DOC.3.2 — Markdown-Renderer für das User-Handbuch.
//
// Liest den Markdown-Body eines Handbuch-Eintrags (siehe `./index.ts`)
// und produziert (a) HTML zum Einsetzen via `{@html}` und (b) eine
// flache Table-of-Contents-Liste aus h2/h3-Headings für die rechte
// Spalte des `/help`-Layouts.
//
// Eigenschaften:
// - `marked` als gebündelte JS-Dep (kein CDN, kein Netz). Marked v15-API.
// - Eigener Heading-Renderer setzt deutsche-aware Slug-IDs und sammelt
//   die TOC-Einträge. Doppelte Slugs werden mit `-1`, `-2`, … aufgelöst.
// - Eigener Image-Renderer löst `![alt](img/<name>)` über `import.meta.glob`
//   zur Build-Zeit auf einen Asset-URL aus
//   `src-tauri/resources/handbook/img/` auf. Fehlt das Bild zur Build-Zeit
//   (häufig in v1.0-Pre-Tag, weil G2-DOC.2.8 die Screenshots erst noch
//   nachliefert), wird der Original-Pfad durchgelassen — das Frontend
//   zeigt das standardisierte Browser-broken-image-Icon, was Manuel
//   sofort als „fehlender Screenshot" liest.
// - Eigener Link-Renderer markiert externe (`https?://…`) Links mit
//   `target="_blank"` und `rel="noopener noreferrer"`. Interne Anker
//   (`#…`) und SPA-Routen (`/help/…`, etc.) bleiben normale Links —
//   `<a href>` löst in SvelteKit den Client-Side-Router aus.
//
// Bewusst NICHT in 3.2:
// - Syntax-Highlighting (Bundle-Größe, im PRD nicht gefordert).
// - DOMPurify (Eigen-Bundle, read-only zur Laufzeit, kein User-Input).
//   Marked default-escaped Text-Tokens; eingebettetes HTML im Markdown
//   wird unverändert durchgereicht (`gfm: true` ist Standard). Falls
//   wir Inline-HTML später unterbinden wollen: `mangle/sanitize` ist
//   ab marked v8 entfernt; dann müsste DOMPurify dazu.

import { Marked, type Tokens } from "marked";

export type TocLevel = 2 | 3;

export interface TocEntry {
  id: string;
  text: string;
  level: TocLevel;
}

export interface RenderResult {
  html: string;
  toc: TocEntry[];
}

// --- Bild-Auflösung -------------------------------------------------------

// Vite-Glob: jedes Bild aus dem Handbuch-Bundle als URL-Asset. Eager,
// damit der Resolver synchron auf einer Map arbeiten kann. Filter auf
// die typischen Bild-Endungen, inkl. Groß- und Kleinschreibung (Windows-
// Dateisysteme matchen sonst je nach Vite-Version inkonsistent).
const IMAGE_URLS = import.meta.glob(
  "../../../src-tauri/resources/handbook/img/*.{png,PNG,jpg,JPG,jpeg,JPEG,gif,GIF,svg,SVG,webp,WEBP}",
  { eager: true, query: "?url", import: "default" },
) as Record<string, string>;

const IMAGE_MAP: Map<string, string> = (() => {
  const m = new Map<string, string>();
  for (const [path, url] of Object.entries(IMAGE_URLS)) {
    const name = path.split("/").pop();
    if (name) m.set(name, url);
  }
  return m;
})();

function resolveImage(href: string | null | undefined): string {
  if (!href) return "";
  // Externe oder Data-URLs unverändert lassen.
  if (/^(https?:|data:)/i.test(href)) return href;
  // Wir akzeptieren `img/foo.png`, `./img/foo.png` und nacktes `foo.png`.
  const name = href.replace(/^.*[/\\]/, "");
  return IMAGE_MAP.get(name) ?? href;
}

// --- Slug- und HTML-Helfer ------------------------------------------------

/**
 * Deutsche-aware Slugifier für Heading-IDs.
 *
 * Ä/Ö/Ü → ae/oe/ue, ß → ss, alles übrige Nicht-Wort-Zeichen wird zu
 * Space normalisiert und mit `-` zusammengezogen. Bewusst lossy, aber
 * deterministisch — gleiches Heading liefert immer denselben Slug.
 */
export function slugifyHeading(text: string): string {
  return text
    .toLowerCase()
    .replace(/ä/g, "ae")
    .replace(/ö/g, "oe")
    .replace(/ü/g, "ue")
    .replace(/ß/g, "ss")
    .replace(/[^a-z0-9\s-]+/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .join("-");
}

function stripHtml(html: string): string {
  return html
    .replace(/<[^>]*>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&[^;]+;/g, " ")
    .trim();
}

function escapeAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

// --- Render-Entry ---------------------------------------------------------

/**
 * Rendert den Markdown-Body eines Handbuch-Eintrags zu HTML + TOC.
 *
 * Jeder Aufruf öffnet eine frische `Marked`-Instanz und einen frischen
 * Slug-Counter, damit Render-Aufrufe nicht ineinander hineinleaken. Die
 * Funktion ist synchron — wir setzen `async: false` an `parse()`, weil
 * unsere Renderer-Hooks selbst synchron sind.
 */
export function renderHandbookMarkdown(body: string): RenderResult {
  const toc: TocEntry[] = [];
  const slugCounts = new Map<string, number>();

  const m = new Marked({ gfm: true, breaks: false });

  m.use({
    renderer: {
      heading(token: Tokens.Heading): string {
        // `this.parser.parseInline` rendert Inline-Tokens (Bold, Italic,
        // Code, Links) im Header-Text. Ohne diesen Aufruf wären nested
        // Markdown-Konstrukte im Header roh.
        const innerHtml = this.parser.parseInline(token.tokens);
        const plain = stripHtml(innerHtml);
        const base = slugifyHeading(plain) || `kapitel-${toc.length + 1}`;
        const seen = slugCounts.get(base) ?? 0;
        const id = seen === 0 ? base : `${base}-${seen}`;
        slugCounts.set(base, seen + 1);
        if (token.depth === 2 || token.depth === 3) {
          toc.push({ id, text: plain, level: token.depth });
        }
        return `<h${token.depth} id="${id}">${innerHtml}</h${token.depth}>\n`;
      },
      image(token: Tokens.Image): string {
        const src = resolveImage(token.href);
        const titleAttr = token.title
          ? ` title="${escapeAttr(token.title)}"`
          : "";
        const altAttr = escapeAttr(token.text ?? "");
        return `<img src="${escapeAttr(src)}" alt="${altAttr}"${titleAttr} loading="lazy">`;
      },
      link(token: Tokens.Link): string {
        const inner = this.parser.parseInline(token.tokens);
        const href = (token.href ?? "").trim();
        const titleAttr = token.title
          ? ` title="${escapeAttr(token.title)}"`
          : "";
        const isExternal = /^https?:/i.test(href);
        if (isExternal) {
          return `<a href="${escapeAttr(href)}" target="_blank" rel="noopener noreferrer"${titleAttr}>${inner}</a>`;
        }
        return `<a href="${escapeAttr(href)}"${titleAttr}>${inner}</a>`;
      },
    },
  });

  const html = m.parse(body, { async: false }) as string;
  return { html, toc };
}

// --- FAQ-Transformation (G2-DOC.3.4) -------------------------------------
//
// Wandelt einen frisch gerenderten Handbuch-Body in eine Accordion-Ansicht
// um: jedes `<h2 id="…">FRAGE</h2>`-Heading wird zum `<summary>` eines
// `<details class="faq" id="faq-…">`-Blocks, das alles bis zum nächsten
// `<h2 …>` oder Dokument-Ende einschließt. Die ID-Präfix `faq-` ist die im
// PRD §G2-DOC.3.4 festgelegte Deeplink-Konvention (`/help/faq#faq-<slug>`).
//
// Bewusst NICHT angefasst:
// - `<h3>`-Headings innerhalb einer Antwort. Die TOC behält sie unverändert.
// - Preamble vor der ersten `<h2>`. Sie bleibt sichtbar als Fließtext.
//
// Die Funktion ist tolerant: ohne `<h2>` (z. B. eine FAQ-Seite ohne
// Fragen — Edge-Case) gibt sie HTML + TOC unverändert zurück.
export function transformAsFaq(html: string, toc: TocEntry[]): RenderResult {
  // TOC: nur h2-Einträge bekommen das `faq-`-Präfix, h3 bleiben unverändert.
  const newToc: TocEntry[] = toc.map((e) =>
    e.level === 2 ? { ...e, id: `faq-${e.id}` } : e,
  );

  const firstH2 = html.search(/<h2\s+id="/i);
  if (firstH2 === -1) {
    return { html, toc: newToc };
  }

  const out: string[] = [];
  if (firstH2 > 0) out.push(html.substring(0, firstH2));

  const rest = html.substring(firstH2);
  const re = /<h2\s+id="([^"]+)">([\s\S]*?)<\/h2>/g;
  const heads: { idx: number; len: number; id: string; inner: string }[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(rest)) !== null) {
    heads.push({ idx: m.index, len: m[0].length, id: m[1], inner: m[2] });
  }
  for (let i = 0; i < heads.length; i++) {
    const head = heads[i];
    const bodyStart = head.idx + head.len;
    const bodyEnd = i + 1 < heads.length ? heads[i + 1].idx : rest.length;
    const body = rest.substring(bodyStart, bodyEnd);
    const id = `faq-${head.id}`;
    // `id` sitzt auf dem `<details>`; das ist der Anker-Empfänger. Das
    // ursprüngliche `<h2>` verliert seine ID, damit es im Dokument keinen
    // doppelten Anker mit gleicher Adresse gibt.
    out.push(
      `<details class="faq" id="${id}"><summary><span class="faq-q">${head.inner}</span></summary><div class="faq-a">${body}</div></details>`,
    );
  }
  return { html: out.join("\n"), toc: newToc };
}
