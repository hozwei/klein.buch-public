// ADR 0042 Nr. 16 (U7-A02): Die Kopfdaten im Start-Bundle (`meta.ts`, Vite-Plugin
// `klein-buch-handbook-meta`) passen zu den Markdown-Dateien und zum vollen Index
// unter /help, und sie tragen keinen Seitentext.

import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { handbookFrontMatter } from "../../../vite.config.js";
import { getMeta, listMeta, listMetaByCategory } from "./meta.ts";
import { getEntry, listByCategory, listEntries } from "./index.ts";
import { slugifyHeading } from "./slug.ts";
import { slugifyHeading as slugifyFromRender } from "./render.ts";

const HANDBOOK = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..", "..", "..", "src-tauri", "resources", "handbook",
);

/** Slug → Titel aus den Dateien, unabhängig vom Parser gelesen. */
function titlesFromFiles(): Map<string, string> {
  const out = new Map<string, string>();
  for (const name of readdirSync(HANDBOOK)) {
    if (!name.endsWith(".md") || name.toLowerCase() === "readme.md") continue;
    const raw = readFileSync(path.join(HANDBOOK, name), "utf8");
    const title = /^title:[ \t]*(.+)$/m.exec(handbookFrontMatter(raw))?.[1].trim() ?? "";
    out.set(name.replace(/\.md$/, ""), title.replace(/^(["'])(.*)\1$/, "$2"));
  }
  return out;
}

describe("Handbuch-Kopfdaten (meta.ts)", () => {
  it("enthält jede Markdown-Datei mit Slug und Titel", () => {
    const files = titlesFromFiles();
    expect(files.size).toBeGreaterThan(10);
    const meta = new Map(listMeta().map((m) => [m.slug, m.title]));
    expect(meta).toEqual(files);
  });

  it("stimmt mit dem vollen Index überein (Reihenfolge, Felder), ohne Text", () => {
    const full = listEntries().map(({ body: _body, ...m }) => m);
    expect(listMeta()).toEqual(full);
    for (const m of listMeta()) {
      expect(m).not.toHaveProperty("body");
      expect(getMeta(m.slug)).toEqual(m);
      expect(getEntry(m.slug)?.title).toBe(m.title);
    }
    expect(getMeta("gibt-es-nicht")).toBeUndefined();
    const strip = (gs: { category: string; label: string; entries: { slug: string }[] }[]) =>
      gs.map((g) => ({ ...g, entries: g.entries.map((e) => e.slug) }));
    expect(strip(listMetaByCategory())).toEqual(strip(listByCategory()));
  });

  it("das Plugin liefert nur den Front-Matter-Block", () => {
    const fm = handbookFrontMatter("---\nslug: a\ntitle: A\n---\n\n# A\n\nText\n");
    expect(fm).toBe("---\nslug: a\ntitle: A\n---\n");
    expect(handbookFrontMatter("\uFEFF---\r\nslug: a\r\n---\r\nText")).toBe("---\r\nslug: a\r\n---\r\n");
    expect(handbookFrontMatter("# Ohne Kopf\n")).toBe("");
  });

  it("slug.ts und render.ts liefern denselben Slugifier", () => {
    expect(slugifyFromRender).toBe(slugifyHeading);
    expect(slugifyHeading("Größe & Übersicht (§19)")).toBe("groesse-uebersicht-19");
  });
});
