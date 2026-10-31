// Handbuch-Kopfdaten für das Start-Bundle (ADR 0042 Nr. 16, U7-A02).
//
// `HelpAnchor` steckt über `BackupGate` im Root-Layout und braucht nur den Titel
// einer Seite. Früher importierte er `index.ts`, das alle Seiten als Rohtext lädt
// (dazu `render.ts` mit `marked`) — das ganze Handbuch lag so im Start-Bundle.
// Hier lädt der glob jede Datei mit der Abfrage `?kbmeta`; das Vite-Plugin
// `handbookMeta` (vite.config.js) liefert dafür nur den Front-Matter-Block, ohne
// Text. Die Inhalte lädt nur /help (`index.ts`, Route-Chunk). Test: `meta.test.ts`.

import {
  groupHandbook,
  parseHandbookFile,
  sortHandbook,
  type HandbookCategoryGroup,
  type HandbookMeta,
} from "./frontmatter";

export type { HandbookMeta } from "./frontmatter";

const metaFiles = import.meta.glob(
  "../../../src-tauri/resources/handbook/*.md",
  { eager: true, query: "?kbmeta", import: "default" },
) as Record<string, string>;

const META: HandbookMeta[] = sortHandbook(
  Object.entries(metaFiles)
    .map(([path, fm]) => parseHandbookFile(path, fm)?.meta)
    .filter((m): m is HandbookMeta => m !== undefined),
);
const BY_SLUG: Map<string, HandbookMeta> = new Map(META.map((m) => [m.slug, m]));

/** Kopfdaten aller Seiten, sortiert nach Kategorie + `order`. */
export function listMeta(): HandbookMeta[] {
  return META;
}

/** Kopfdaten einer Seite per Slug (oder `undefined`). */
export function getMeta(slug: string): HandbookMeta | undefined {
  return BY_SLUG.get(slug);
}

/** Kopfdaten gruppiert nach Kategorie (Kapitelbaum unter /help). */
export function listMetaByCategory(): HandbookCategoryGroup[] {
  return groupHandbook(META);
}
