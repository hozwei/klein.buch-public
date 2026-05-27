// G2-DOC.3.1 — Handbuch-Index (Front-Matter-only, kein Body-Render).
//
// Lädt zur Build-Zeit alle `*.md`-Files unter
// `src-tauri/resources/handbook/` via Vite-glob (`import.meta.glob`, raw),
// parst den YAML-Front-Matter (5 Pflichtfelder) mit einem schlanken
// Eigen-Parser und liefert eine sortierte Liste `HandbookEntry[]`.
//
// Bewusst KEINE JS-Dependency (`gray-matter` o. ä.): die App ist local-first,
// jede zusätzliche transitive Dep blähte das Bundle auf und brächte
// Supply-Chain-Risiko für 5 simple Felder. Die echte Verträge-Prüfung läuft
// Rust-seitig (`handbook_resources_test`, `handbook_footer_test`).
//
// Den Markdown-Body geben wir hier als String mit (für den späteren Renderer in
// G2-DOC.3.2). In 3.1 wird er von der `[slug]`-Stub-Seite einfach als `<pre>`
// ausgegeben.

import {
  HANDBOOK_CATEGORIES,
  HANDBOOK_CATEGORY_LABELS,
  isHandbookCategory,
  type HandbookCategory,
} from "./categories";

export interface HandbookEntry {
  slug: string;
  title: string;
  category: HandbookCategory;
  order: number;
  keywords: string[];
  body: string; // alles nach dem schließenden `---`
}

export interface HandbookCategoryGroup {
  category: HandbookCategory;
  label: string;
  entries: HandbookEntry[];
}

// Vite-glob — relativer Pfad vom Frontend-Root (`klein-buch/`) aus auf den
// Resource-Ordner unter `src-tauri/`. README.md ist im Pattern ausgenommen,
// um sich gar nicht erst eine Sonderbehandlung einzuhandeln.
const rawFiles = import.meta.glob(
  "../../../src-tauri/resources/handbook/*.md",
  { eager: true, query: "?raw", import: "default" },
) as Record<string, string>;

const FRONT_MATTER_RE = /^---\s*\r?\n([\s\S]*?)\r?\n---\s*\r?\n([\s\S]*)$/;

function parseScalar(raw: string): string {
  const t = raw.trim();
  if (
    (t.startsWith('"') && t.endsWith('"')) ||
    (t.startsWith("'") && t.endsWith("'"))
  ) {
    return t.slice(1, -1);
  }
  return t;
}

function parseList(raw: string): string[] {
  const t = raw.trim();
  if (!(t.startsWith("[") && t.endsWith("]"))) return [];
  return t
    .slice(1, -1)
    .split(",")
    .map((s) => parseScalar(s))
    .filter((s) => s.length > 0);
}

function parseFrontMatter(yaml: string): Record<string, string> {
  // Wir unterstützen NUR die Form `key: value` (eine Zeile pro Feld). Genau das
  // schreibt das README für Handbuch-Files vor. Verschachtelte Strukturen oder
  // mehrzeilige YAML-Werte sind nicht zugelassen.
  const out: Record<string, string> = {};
  for (const line of yaml.split(/\r?\n/)) {
    if (!line.trim() || line.trim().startsWith("#")) continue;
    const idx = line.indexOf(":");
    if (idx < 0) continue;
    const key = line.slice(0, idx).trim();
    const value = line.slice(idx + 1);
    if (key) out[key] = value;
  }
  return out;
}

function fileBasename(path: string): string {
  const last = path.split("/").pop() ?? path;
  return last.replace(/\.md$/i, "");
}

function buildEntry(path: string, raw: string): HandbookEntry | null {
  // README.md überspringen — sie ist Doku-Konvention, keine Handbuch-Seite.
  if (fileBasename(path).toLowerCase() === "readme") return null;

  const match = raw.match(FRONT_MATTER_RE);
  if (!match) {
    console.warn(`[handbook] kein Front-Matter in ${path}, übersprungen`);
    return null;
  }
  const [, yaml, body] = match;
  const fm = parseFrontMatter(yaml);

  const slug = parseScalar(fm.slug ?? "");
  const title = parseScalar(fm.title ?? "");
  const categoryRaw = parseScalar(fm.category ?? "");
  const orderRaw = parseScalar(fm.order ?? "");
  const keywords = parseList(fm.keywords ?? "[]");

  if (!slug || !title || !categoryRaw || !orderRaw) {
    console.warn(
      `[handbook] unvollständiges Front-Matter in ${path} (slug/title/category/order fehlt), übersprungen`,
    );
    return null;
  }
  if (!isHandbookCategory(categoryRaw)) {
    console.warn(
      `[handbook] unbekannte Kategorie "${categoryRaw}" in ${path}, übersprungen`,
    );
    return null;
  }
  const order = Number.parseInt(orderRaw, 10);
  if (Number.isNaN(order)) {
    console.warn(
      `[handbook] order ist keine Zahl ("${orderRaw}") in ${path}, übersprungen`,
    );
    return null;
  }
  // Slug-Eindeutigkeits-Check (filename == slug) — Rust-Test prüft das auch,
  // hier nur eine zweite Sicherung gegen Fehlbestückung im Bundle.
  if (fileBasename(path) !== slug) {
    console.warn(
      `[handbook] slug "${slug}" stimmt nicht mit Dateinamen überein in ${path}, übersprungen`,
    );
    return null;
  }

  return { slug, title, category: categoryRaw, order, keywords, body };
}

function loadEntries(): HandbookEntry[] {
  const items: HandbookEntry[] = [];
  for (const [path, raw] of Object.entries(rawFiles)) {
    const entry = buildEntry(path, raw);
    if (entry) items.push(entry);
  }
  // Stabile Sortierung: Kategorie-Index zuerst (= Anzeigereihenfolge im
  // Sidebar-Kapitelbaum), dann `order`, dann Titel als Tiebreaker.
  const catIndex = (c: HandbookCategory) => HANDBOOK_CATEGORIES.indexOf(c);
  items.sort((a, b) => {
    const ci = catIndex(a.category) - catIndex(b.category);
    if (ci !== 0) return ci;
    if (a.order !== b.order) return a.order - b.order;
    return a.title.localeCompare(b.title, "de");
  });
  return items;
}

const ENTRIES: HandbookEntry[] = loadEntries();
const BY_SLUG: Map<string, HandbookEntry> = new Map(
  ENTRIES.map((e) => [e.slug, e]),
);

/** Alle Handbuch-Einträge, sortiert nach Kategorie + `order`. */
export function listEntries(): HandbookEntry[] {
  return ENTRIES;
}

/** Einen Eintrag per Slug holen (oder `undefined`). */
export function getEntry(slug: string): HandbookEntry | undefined {
  return BY_SLUG.get(slug);
}

/**
 * Einträge gruppiert nach Kategorie in fixer Anzeigereihenfolge.
 * Leere Kategorien werden mit `entries: []` mitgeliefert, damit der
 * Renderer entscheiden kann, ob er sie ausblendet.
 */
export function listByCategory(): HandbookCategoryGroup[] {
  const groups: HandbookCategoryGroup[] = HANDBOOK_CATEGORIES.map((cat) => ({
    category: cat,
    label: HANDBOOK_CATEGORY_LABELS[cat],
    entries: [],
  }));
  for (const entry of ENTRIES) {
    const group = groups.find((g) => g.category === entry.category);
    if (group) group.entries.push(entry);
  }
  return groups;
}

/** Alle Slugs — wird vom Prerender (`adapter-static`) gebraucht. */
export function listSlugs(): string[] {
  return ENTRIES.map((e) => e.slug);
}
