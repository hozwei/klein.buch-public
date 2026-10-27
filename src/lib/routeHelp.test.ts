// U4 (V1-19, V2-01/V4-01): F1 führt zum passenden Handbuch-Kapitel, und jede Route
// hat einen Ersatz-Seitentitel. Statische Prüfung gegen Handbuch-Dateien und
// Routenordner.
import { describe, expect, it } from "vitest";
import { existsSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { helpSlugForRoute, ROUTE_HELP_SLUGS } from "./routeHelp.ts";
import { documentTitle, routeTitle } from "./pageTitle.svelte.ts";

const LIB = path.dirname(fileURLToPath(import.meta.url));
const HANDBOOK = path.join(LIB, "..", "..", "src-tauri", "resources", "handbook");
const ROUTES = path.join(LIB, "..", "routes");

/** Route-IDs aller Seiten (`+page.svelte`) unter src/routes. */
function routeIds(dir = ROUTES, prefix = ""): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) out.push(...routeIds(full, `${prefix}/${name}`));
    else if (name === "+page.svelte") out.push(prefix || "/");
  }
  return out;
}

describe("F1 → Handbuch", () => {
  it("jeder Slug ist eine Handbuch-Datei", () => {
    for (const slug of ROUTE_HELP_SLUGS) {
      expect(existsSync(path.join(HANDBOOK, `${slug}.md`)), slug).toBe(true);
    }
  });

  it("längstes Präfix gewinnt, sonst Startseite", () => {
    expect(helpSlugForRoute("/invoices/[id]/payment")).toBe("zahlung-erfassen");
    expect(helpSlugForRoute("/invoices/[id]")).toBe("rechnungen");
    expect(helpSlugForRoute("/settings/backup-log")).toBe("backup-und-wiederherstellen");
    expect(helpSlugForRoute("/settings/legal")).toBe("einstellungen");
    expect(helpSlugForRoute("/")).toBeNull();
    expect(helpSlugForRoute(null)).toBeNull();
    // Präfix nur an Pfadgrenzen
    expect(helpSlugForRoute("/expenses-x")).toBeNull();
  });
});

describe("Seitentitel", () => {
  it("jede Route hat einen Ersatztitel", () => {
    for (const id of routeIds()) {
      expect(routeTitle(id), id).toBeTruthy();
    }
  });

  it("Format „Seite – Klein.Buch“", () => {
    expect(documentTitle("Rechnungen")).toBe("Rechnungen – Klein.Buch");
    expect(documentTitle("Klein.Buch einrichten")).toBe("Klein.Buch einrichten");
    expect(documentTitle("  ")).toBe("Klein.Buch");
    expect(documentTitle(null)).toBe("Klein.Buch");
  });
});
