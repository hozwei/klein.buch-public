// Lauf 2026.11 U4 (ADR 0041 Nr. 212): Vorkomprimierung des Frontend-Builds
// (`precompressDir` aus vite.config.js, Plugin `klein-buch-precompress`). Die
// Auswahl der Variante im Server prüft src-tauri/tests/server_precompressed_test.rs.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { randomBytes } from "node:crypto";
import { tmpdir } from "node:os";
import path from "node:path";
import { brotliDecompressSync, gunzipSync } from "node:zlib";
import { precompressDir } from "../vite.config.js";

let dir = "";
const put = (rel: string, content: string | Buffer) => {
  const file = path.join(dir, rel);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, content);
  return file;
};
const text = (n: number) => "export const kleinBuch = 'Rechnung';\n".repeat(Math.ceil(n / 37)).slice(0, n);

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "kb-precompress-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe("precompressDir", () => {
  it("legt .br und .gz an, die zur Datei dekomprimieren", async () => {
    const files = [
      "index.html",
      "_app/immutable/chunks/a.js",
      "_app/immutable/assets/0.css",
      "favicon.svg",
      "_app/env.json",
      "manifest.webmanifest",
    ].map((rel) => put(rel, text(4096)));
    const stats = await precompressDir(dir);
    expect(stats.files).toBe(files.length);
    for (const file of files) {
      const raw = readFileSync(file);
      const br = readFileSync(file + ".br");
      const gz = readFileSync(file + ".gz");
      expect(br.length, file).toBeLessThan(raw.length);
      expect(gz.length, file).toBeLessThan(raw.length);
      expect(brotliDecompressSync(br).equals(raw), file).toBe(true);
      expect(gunzipSync(gz).equals(raw), file).toBe(true);
    }
  });

  it("lässt kleine Dateien, andere Endungen und Unkomprimierbares aus", async () => {
    const small = put("_app/version.json", text(1024));
    const png = put("app-icon.png", text(4096));
    const map = put("_app/immutable/chunks/a.js.map", text(4096));
    const random = put("_app/immutable/chunks/zufall.js", randomBytes(4096));
    // eine Variante aus einem früheren Lauf verschwindet
    put("_app/version.json.gz", "alt");
    await precompressDir(dir);
    for (const file of [small, png, map, random]) {
      expect(existsSync(file + ".br"), file).toBe(false);
      expect(existsSync(file + ".gz"), file).toBe(false);
    }
  });

  it("lässt ausgenommene Unterordner der ersten Ebene aus", async () => {
    const legal = put("legal/js-licenses.json", text(4096));
    const nested = put("_app/legal/b.js", text(4096));
    await precompressDir(dir, { skip: ["legal"] });
    expect(existsSync(legal + ".br")).toBe(false);
    expect(existsSync(legal + ".gz")).toBe(false);
    expect(existsSync(nested + ".br")).toBe(true);
    expect(existsSync(nested + ".gz")).toBe(true);
  });
});
