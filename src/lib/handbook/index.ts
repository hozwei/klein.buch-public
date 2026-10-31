// G2-DOC.3.1 — Handbuch-Index mit Seitentext.
//
// Lädt zur Build-Zeit alle `*.md`-Files unter
// `src-tauri/resources/handbook/` via Vite-glob (`import.meta.glob`, raw),
// parst den YAML-Front-Matter (5 Pflichtfelder, `./frontmatter.ts`) und liefert
// eine sortierte Liste `HandbookEntry[]` samt Markdown-Body.
//
// ADR 0042 Nr. 16 (U7-A02): Dieses Modul enthält das ganze Handbuch und darf nur
// aus den Routen unter /help importiert werden (eigener, nachgeladener Chunk).
// Wer im Start-Bundle nur Titel oder Slugs braucht, nimmt `./meta.ts`.

import {
  groupHandbook,
  parseHandbookFile,
  sortHandbook,
  type HandbookCategoryGroup as Group,
  type HandbookMeta,
} from "./frontmatter";

export interface HandbookEntry extends HandbookMeta {
  body: string; // alles nach dem schließenden `---`
}

export type HandbookCategoryGroup = Group<HandbookEntry>;

// Vite-glob — relativer Pfad vom Frontend-Root (`klein-buch/`) aus auf den
// Resource-Ordner unter `src-tauri/`. README.md wird beim Parsen übersprungen.
const rawFiles = import.meta.glob(
  "../../../src-tauri/resources/handbook/*.md",
  { eager: true, query: "?raw", import: "default" },
) as Record<string, string>;

function loadEntries(): HandbookEntry[] {
  const items: HandbookEntry[] = [];
  for (const [path, raw] of Object.entries(rawFiles)) {
    const parsed = parseHandbookFile(path, raw);
    if (parsed) items.push({ ...parsed.meta, body: parsed.body });
  }
  return sortHandbook(items);
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
 * Leere Kategorien werden mit `entries: []` mitgeliefert.
 */
export function listByCategory(): HandbookCategoryGroup[] {
  return groupHandbook(ENTRIES);
}

/** Alle Slugs — wird vom Prerender (`adapter-static`) gebraucht. */
export function listSlugs(): string[] {
  return ENTRIES.map((e) => e.slug);
}
