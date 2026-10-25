// Lauf F3c (ADR 0040 Nr. 52/58): Jeder Off-Site-Status außer „ok“ aus dem
// Vertrag in `$lib/api` (`offsiteStatus`) hat einen Hinweis auf der Übersicht
// und auf der Seite „Backups“, auf die die Übersicht verlinkt. Statische
// Prüfung der Quelltexte (die Gestaltung macht der UI/UX-Lauf).

import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROUTES = path.dirname(fileURLToPath(import.meta.url));
const read = (...p: string[]) => readFileSync(path.join(ROUTES, ...p), "utf8");

function statusValues(): string[] {
  const api = read("..", "lib", "api.ts");
  const m = api.match(/offsiteStatus:\s*([^;]+);/);
  expect(m, "offsiteStatus im Vertrag").toBeTruthy();
  return [...m![1].matchAll(/"([a-z]+)"/g)].map((x) => x[1]);
}

describe("Off-Site-Status", () => {
  it("Vertrag kennt missing, failing, stale und ok", () => {
    expect(statusValues().sort()).toEqual(["failing", "missing", "ok", "stale"]);
  });

  it("Seite Backups und Übersicht zeigen jeden Status außer ok", () => {
    const settings = read("settings", "backup", "+page.svelte");
    const dashboard = read("+page.svelte");
    for (const s of statusValues().filter((v) => v !== "ok")) {
      expect(settings, `Backups: ${s}`).toContain(`settings.offsiteStatus === "${s}"`);
      expect(dashboard, `Übersicht: ${s}`).toContain(`offsiteStatus === "${s}"`);
    }
  });
});
