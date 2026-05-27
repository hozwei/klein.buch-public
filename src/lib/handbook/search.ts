// G2-DOC.3.3 — Volltextsuche über das Handbuch.
//
// Lazy gebauter MiniSearch-Index über alle Handbuch-Einträge aus
// `./index.ts`. Felder: `title` (Boost 3), `keywords` (Boost 2), `body`
// (Plain-Text aus dem Markdown extrahiert). Suche AND-kombiniert,
// Präfix-Match an, Fuzzy 0.2 — verzeiht Tippfehler, ohne Treffer-
// Wolken zu produzieren.
//
// Eigenschaften:
// - Lokal gebündelt (`minisearch` als pnpm-Dep, kein CDN, kein Netz).
//   Klein-Buch ist local-first; jede Online-Suche wäre ein Bruch.
// - Index wird beim ersten `searchHandbook()`-Aufruf gebaut und für die
//   Sitzung gecacht. Die Eingangs-Daten (`listEntries()`) sind build-
//   zeitlich eingefroren, daher gibt es keinen Refresh-Pfad.
// - Snippet-Builder schneidet einen Kontext-Fenster-Slice um das erste
//   Term-Match heraus, HTML-escaped den Slice und wickelt die Terme
//   in `<mark>`-Tags. Wird im `+layout.svelte` per `{@html}` eingesetzt.
//   Sicher, weil die Quelle ausschließlich aus dem read-only Handbook-
//   Bundle stammt und der Escape vor dem Highlight läuft.
//
// Bewusst NICHT:
// - Stemming/Stoppwörter: MiniSearch unterstützt das, der Aufwand für
//   Deutsch lohnt sich aber bei ~40 Seiten nicht — Präfix + Fuzzy
//   reichen.
// - Inkrementelles Indexieren: wir lesen einmal ein.
// - Persistenz im LocalStorage: artifact-Verbot in artifacts und
//   Cowork-CLAUDE-Regeln; sowieso wäre der Re-Build < 50 ms.

import MiniSearch, { type SearchResult } from "minisearch";

import {
  HANDBOOK_CATEGORY_LABELS,
  type HandbookCategory,
} from "./categories";
import { listEntries } from "./index";

export interface HandbookHit {
  slug: string;
  title: string;
  category: HandbookCategory;
  categoryLabel: string;
  /** HTML-Snippet mit `<mark>`-umschlossenen Treffer-Termen. */
  snippet: string;
  /** MiniSearch-Score (höher = relevanter). */
  score: number;
}

// --- Markdown → Plain-Text ------------------------------------------------

/**
 * Sehr schlanker Markdown-Stripper. Genau genug für einen Such-Index
 * (Wörter raus, Layout weg), nicht stark genug für einen Renderer.
 * Spezial-Fälle, die wir behandeln, weil sie sonst die Suche stören:
 *
 * - Code-Fences (```…```) komplett entfernen (sonst tauchen
 *   Sprachen-Tags wie `ts` als Treffer auf).
 * - Inline-Code-Backticks abstreifen.
 * - Bilder `![alt](src)` ganz entfernen.
 * - Links `[text](href)` durch `text` ersetzen.
 * - Heading-/Emphasis-/Quote-/List-Marker entfernen.
 * - Inline-HTML-Tags entfernen.
 * - Mehrfach-Whitespace zusammenziehen.
 */
function markdownToPlain(body: string): string {
  return body
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/^>\s?/gm, "")
    .replace(/[*_~]/g, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

// --- HTML- und Regex-Escape ----------------------------------------------

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// --- Index-Aufbau (lazy, einmal pro Sitzung) -----------------------------

interface IndexedDoc {
  slug: string;
  title: string;
  body: string;
  keywords: string;
  category: HandbookCategory;
}

interface DocSidecar {
  plain: string;
}

let _mini: MiniSearch<IndexedDoc> | null = null;
const _sidecars: Map<string, DocSidecar> = new Map();

function buildIndex(): MiniSearch<IndexedDoc> {
  if (_mini) return _mini;
  const mini = new MiniSearch<IndexedDoc>({
    idField: "slug",
    fields: ["title", "keywords", "body"],
    storeFields: ["slug", "title", "category"],
    searchOptions: {
      boost: { title: 3, keywords: 2 },
      prefix: true,
      fuzzy: 0.2,
      combineWith: "AND",
    },
  });
  for (const entry of listEntries()) {
    const plain = markdownToPlain(entry.body);
    _sidecars.set(entry.slug, { plain });
    mini.add({
      slug: entry.slug,
      title: entry.title,
      body: plain,
      keywords: entry.keywords.join(" "),
      category: entry.category,
    });
  }
  _mini = mini;
  return mini;
}

// --- Snippet-Builder ------------------------------------------------------

const SNIPPET_RADIUS = 70;

interface FirstMatch {
  index: number;
  length: number;
}

/**
 * Findet das erste Vorkommen irgendeines Terms im Plain-Text. Längere
 * Terme gewinnen bei Gleichstand, damit z. B. „Passphrase" vor „pass"
 * als Anker dient.
 */
function findFirstMatch(plain: string, terms: string[]): FirstMatch | null {
  let best: FirstMatch | null = null;
  for (const t of terms) {
    if (!t) continue;
    const re = new RegExp(escapeRegex(t), "i");
    const m = plain.match(re);
    if (!m || m.index === undefined) continue;
    if (
      !best ||
      m.index < best.index ||
      (m.index === best.index && m[0].length > best.length)
    ) {
      best = { index: m.index, length: m[0].length };
    }
  }
  return best;
}

function highlight(html: string, terms: string[]): string {
  // Längere Terme zuerst — verhindert, dass kürzere Terme bereits
  // markierte Treffer doppelt umschließen (innerhalb von `<mark>`).
  const ordered = [...new Set(terms.filter(Boolean))].sort(
    (a, b) => b.length - a.length,
  );
  let out = html;
  for (const t of ordered) {
    const safe = escapeRegex(escapeHtml(t));
    // Negative Lookbehind/Lookahead: nicht innerhalb eines bereits
    // gesetzten `<mark>…</mark>` erneut markieren.
    const re = new RegExp(
      `(?<!<mark>)(${safe})(?![^<]*</mark>)`,
      "gi",
    );
    out = out.replace(re, "<mark>$1</mark>");
  }
  return out;
}

function buildSnippet(plain: string, terms: string[]): string {
  if (!plain) return "";
  const hit = findFirstMatch(plain, terms);
  if (!hit) {
    const head = plain.slice(0, SNIPPET_RADIUS * 2);
    return escapeHtml(head) + (plain.length > head.length ? "…" : "");
  }
  const start = Math.max(0, hit.index - SNIPPET_RADIUS);
  const end = Math.min(plain.length, hit.index + hit.length + SNIPPET_RADIUS);
  const slice = plain.slice(start, end);
  const safe = escapeHtml(slice);
  return (
    (start > 0 ? "…" : "") +
    highlight(safe, terms) +
    (end < plain.length ? "…" : "")
  );
}

// --- Öffentliche API ------------------------------------------------------

export const SEARCH_MIN_QUERY_LENGTH = 2;
export const SEARCH_DEFAULT_LIMIT = 20;

/**
 * Sucht im Handbuch. Liefert eine nach Score sortierte, gecappte Liste.
 * Bei zu kurzer Query (`< SEARCH_MIN_QUERY_LENGTH`) leeres Array — der
 * Aufrufer soll dann den Kapitelbaum zeigen.
 */
export function searchHandbook(
  query: string,
  limit: number = SEARCH_DEFAULT_LIMIT,
): HandbookHit[] {
  const q = query.trim();
  if (q.length < SEARCH_MIN_QUERY_LENGTH) return [];
  const mini = buildIndex();
  const raw = mini.search(q) as Array<SearchResult & { terms?: string[] }>;
  const capped = raw.slice(0, limit);
  return capped.map((r) => {
    const slug = String(r.id);
    const sidecar = _sidecars.get(slug);
    const category = (r.category as HandbookCategory | undefined) ?? "bedienen";
    return {
      slug,
      title: (r.title as string | undefined) ?? slug,
      category,
      categoryLabel: HANDBOOK_CATEGORY_LABELS[category],
      snippet: buildSnippet(sidecar?.plain ?? "", r.terms ?? []),
      score: r.score,
    };
  });
}

/**
 * Reset für Tests / Storybook-Sitzungen — nicht im Produkt verwendet.
 * Erzwingt den Re-Build des Index beim nächsten `searchHandbook()`.
 */
export function _resetIndexForTests(): void {
  _mini = null;
  _sidecars.clear();
}
