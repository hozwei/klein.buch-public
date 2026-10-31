// Front-Matter der Handbuch-Seiten (ADR 0042 Nr. 16, U7-A02).
//
// Gemeinsamer Parser für `index.ts` (ganze Seiten, nur unter /help) und
// `meta.ts` (nur Titel/Slug/Kategorie für das Start-Bundle). Beide lesen dieselben
// Dateien unter `src-tauri/resources/handbook/`; so gibt es keine zweite, von Hand
// gepflegte Titelliste.
//
// Bewusst KEINE JS-Dependency (`gray-matter` o. ä.): jede zusätzliche transitive
// Dep blähte das Bundle auf und brächte Supply-Chain-Risiko für 5 simple Felder.
// Die echte Vertrags-Prüfung läuft Rust-seitig (`handbook_resources_test`,
// `handbook_footer_test`).

import {
  HANDBOOK_CATEGORIES,
  HANDBOOK_CATEGORY_LABELS,
  isHandbookCategory,
  type HandbookCategory,
} from "./categories";

/** Kopfdaten einer Handbuch-Seite (Front-Matter, ohne Text). */
export interface HandbookMeta {
  slug: string;
  title: string;
  category: HandbookCategory;
  order: number;
  keywords: string[];
}

export interface HandbookCategoryGroup<T extends HandbookMeta = HandbookMeta> {
  category: HandbookCategory;
  label: string;
  entries: T[];
}

const FRONT_MATTER_RE = /^---\s*\r?\n([\s\S]*?)\r?\n---\s*(?:\r?\n([\s\S]*))?$/;

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

export function fileBasename(path: string): string {
  const last = path.split("/").pop() ?? path;
  return last.replace(/\.md$/i, "").replace(/\?.*$/, "");
}

/**
 * Liest Front-Matter und Text einer Handbuch-Datei. `null` für README.md und für
 * Dateien mit fehlendem oder fehlerhaftem Front-Matter (mit Warnung).
 */
export function parseHandbookFile(
  path: string,
  raw: string,
): { meta: HandbookMeta; body: string } | null {
  // README.md überspringen — sie ist Doku-Konvention, keine Handbuch-Seite.
  if (fileBasename(path).toLowerCase() === "readme") return null;

  const match = raw.match(FRONT_MATTER_RE);
  if (!match) {
    console.warn(`[handbook] kein Front-Matter in ${path}, übersprungen`);
    return null;
  }
  const [, yaml, body = ""] = match;
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

  return { meta: { slug, title, category: categoryRaw, order, keywords }, body };
}

/**
 * Stabile Sortierung: Kategorie-Index zuerst (= Anzeigereihenfolge im
 * Sidebar-Kapitelbaum), dann `order`, dann Titel als Tiebreaker.
 */
export function sortHandbook<T extends HandbookMeta>(items: T[]): T[] {
  const catIndex = (c: HandbookCategory) => HANDBOOK_CATEGORIES.indexOf(c);
  return items.sort((a, b) => {
    const ci = catIndex(a.category) - catIndex(b.category);
    if (ci !== 0) return ci;
    if (a.order !== b.order) return a.order - b.order;
    return a.title.localeCompare(b.title, "de");
  });
}

/**
 * Einträge gruppiert nach Kategorie in fixer Anzeigereihenfolge. Leere
 * Kategorien werden mit `entries: []` mitgeliefert, damit der Renderer
 * entscheiden kann, ob er sie ausblendet.
 */
export function groupHandbook<T extends HandbookMeta>(
  items: T[],
): HandbookCategoryGroup<T>[] {
  const groups: HandbookCategoryGroup<T>[] = HANDBOOK_CATEGORIES.map((cat) => ({
    category: cat,
    label: HANDBOOK_CATEGORY_LABELS[cat],
    entries: [],
  }));
  for (const entry of items) {
    const group = groups.find((g) => g.category === entry.category);
    if (group) group.entries.push(entry);
  }
  return groups;
}
