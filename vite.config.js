import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";
import { brotliCompress, constants as zlibConstants, gzip } from "node:zlib";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// Entwicklung mit `pnpm dev` (Plan W9): Die SPA kommt vom Vite-Dev-Server,
// API, Dateien, Healthcheck und `/legal/` gehen an den lokalen Rust-Server
// (`klein-buch-server`). Ziel über `KLEIN_BUCH_DEV_API`, Standard
// http://127.0.0.1:8080. Der Server prüft Host und Origin gegen seine
// öffentliche Adresse — für die Entwicklung mit
// `KLEIN_BUCH_PUBLIC_URL=http://127.0.0.1:5173` und
// `KLEIN_BUCH_INSECURE_HTTP=1` starten.
const api = process.env.KLEIN_BUCH_DEV_API || "http://127.0.0.1:8080";

// JS-Teil der Lizenzseite `/legal/` (Plan W12): Beim `vite build` werden aus
// den Modulen des tatsächlich ausgelieferten Client-Bundles die npm-Pakete
// ermittelt und mit Version, Lizenz und Lizenztext nach
// `build/legal/js-licenses.json` geschrieben; `legal/build_legal.py` setzt
// daraus den Abschnitt der Seite zusammen.
//
// Vite 8 baut alle Umgebungen in einem Lauf (`buildApp`): SvelteKit zuerst
// die Server-Umgebung (SSR), dann die Client-Umgebung, zuletzt (`buildApp`,
// Reihenfolge „post“) den Adapter, der `build/` leert und neu füllt. Deshalb
// schreibt die Client-Umgebung die Liste zuerst nach `.svelte-kit/`, und ein
// eigener `buildApp`-Schritt nach dem Adapter kopiert sie an ihr Ziel. Die
// SSR-Umgebung zählt nicht: sie wird nicht ausgeliefert.
const JS_LICENSES_STAGE = ".svelte-kit/klein-buch-legal/js-licenses.json";
const JS_LICENSES_OUT = "build/legal/js-licenses.json";
const LICENSE_FILE = /^(licen[cs]e|copying)([-._].*)?$/i;
const NOTICE_FILE = /^notice([-._].*)?$/i;

/**
 * Paketwurzel (`…/node_modules/<name>` bzw. `…/node_modules/@scope/name`) einer Modul-ID, sonst null.
 * @param {string} id
 * @returns {string | null}
 */
export function packageRootOf(id) {
  const file = id.replace(/^\0/, "").split("?")[0].replace(/\\/g, "/");
  const marker = "/node_modules/";
  const at = file.lastIndexOf(marker);
  if (at < 0) return null;
  const parts = file.slice(at + marker.length).split("/");
  const depth = parts[0].startsWith("@") ? 2 : 1;
  if (parts[0] === ".pnpm" || parts.length <= depth) return null;
  return file.slice(0, at + marker.length) + parts.slice(0, depth).join("/");
}

/**
 * @param {any} pkg Inhalt der package.json
 * @returns {string}
 */
function licenseOf(pkg) {
  if (typeof pkg.license === "string") return pkg.license;
  if (pkg.license && typeof pkg.license.type === "string") return pkg.license.type;
  if (Array.isArray(pkg.licenses)) {
    return pkg.licenses
      .map((/** @type {any} */ l) => (typeof l === "string" ? l : l?.type))
      .filter(Boolean)
      .join(" OR ");
  }
  return "";
}

/**
 * @param {string} root
 * @param {RegExp} pattern
 * @returns {string}
 */
function readFiles(root, pattern) {
  return readdirSync(root, { withFileTypes: true })
    .filter((e) => e.isFile() && pattern.test(e.name))
    .map((e) => e.name)
    .sort()
    .map((name) => readFileSync(path.join(root, name), "utf8").replace(/\r\n/g, "\n").trimEnd())
    .join("\n\n");
}

/**
 * @typedef {{ name: string, version: string, license: string, licenseText: string, noticeText?: string }} JsLicense
 */

/**
 * Einträge `{name, version, license, licenseText[, noticeText]}` zu den Modul-IDs eines Bundles.
 * @param {Iterable<string>} moduleIds
 * @returns {{ list: JsLicense[], problems: string[] }}
 */
export function collectJsLicenses(moduleIds) {
  /** @type {Set<string>} */
  const roots = new Set();
  for (const id of moduleIds) {
    const root = packageRootOf(id);
    if (root) roots.add(root);
  }
  /** @type {Map<string, JsLicense>} */
  const entries = new Map();
  /** @type {string[]} */
  const problems = [];
  for (const root of roots) {
    const pkg = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8"));
    const key = `${pkg.name}@${pkg.version}`;
    if (entries.has(key)) continue;
    /** @type {JsLicense} */
    const entry = {
      name: pkg.name,
      version: pkg.version,
      license: licenseOf(pkg),
      licenseText: readFiles(root, LICENSE_FILE),
    };
    const notice = readFiles(root, NOTICE_FILE);
    if (notice) entry.noticeText = notice;
    if (!entry.name || !entry.version || !entry.license || !entry.licenseText) {
      problems.push(`${key} (${root}): Name, Version, Lizenz oder Lizenztext fehlt`);
    }
    entries.set(key, entry);
  }
  const list = [...entries.values()].sort((a, b) =>
    a.name === b.name ? a.version.localeCompare(b.version) : a.name.localeCompare(b.name),
  );
  return { list, problems };
}

/**
 * Vite-Plugins, die `build/legal/js-licenses.json` schreiben (ohne weitere npm-Abhängigkeit).
 * Muss in `plugins` nach `sveltekit()` stehen: Der Kopierschritt läuft als
 * `buildApp` „post“ nach dem des Adapters.
 * @returns {import("vite").Plugin[]}
 */
export function jsLicenses() {
  let root = process.cwd();
  const stage = () => path.join(root, JS_LICENSES_STAGE);
  return [
    {
      name: "klein-buch-js-licenses",
      apply: "build",
      configResolved(config) {
        root = config.root;
      },
      buildApp: {
        order: "pre",
        async handler() {
          rmSync(stage(), { force: true });
        },
      },
      generateBundle(_options, bundle) {
        if (this.environment.name !== "client") return;
        /** @type {string[]} */
        const ids = [];
        for (const chunk of Object.values(bundle)) {
          if (chunk.type !== "chunk") continue;
          for (const [id, info] of Object.entries(chunk.modules)) {
            if (info.renderedLength > 0) ids.push(id);
          }
        }
        const { list, problems } = collectJsLicenses(ids);
        if (problems.length) this.error(`Lizenzangaben unvollständig:\n${problems.join("\n")}`);
        if (!list.length) this.error("Keine npm-Pakete im Client-Bundle gefunden.");
        mkdirSync(path.dirname(stage()), { recursive: true });
        writeFileSync(stage(), JSON.stringify(list, null, 2) + "\n");
      },
    },
    {
      name: "klein-buch-js-licenses-copy",
      apply: "build",
      buildApp: {
        order: "post",
        async handler() {
          if (!existsSync(stage())) {
            throw new Error(`${JS_LICENSES_STAGE} fehlt — der Client-Build hat keine Lizenzliste geschrieben.`);
          }
          const out = path.join(root, JS_LICENSES_OUT);
          mkdirSync(path.dirname(out), { recursive: true });
          copyFileSync(stage(), out);
        },
      },
    },
  ];
}

// Vorkomprimierte Auslieferung (Lauf 2026.11 U4, ADR 0041 Nr. 212): Nach dem
// Adapter liegt neben jeder Text-Datei in `build/` eine `<datei>.br` (Brotli,
// Qualität 11) und eine `<datei>.gz` (gzip -9). Der Server (`ServeDir`) wählt
// sie nach `Accept-Encoding`; er komprimiert selbst nichts. Ausgelassen: Dateien
// bis 1 KiB, Varianten, die nicht kleiner sind, und `build/legal/` (Lizenzliste,
// die das Dockerfile herausnimmt; `/legal/` liefert der Server aus dem
// Ressourcen-Ordner).
const PRECOMPRESS_FILE = /\.(?:js|css|html|svg|json|webmanifest)$/i;
const PRECOMPRESS_MIN_BYTES = 1024;
const PRECOMPRESS_SKIP_DIRS = ["legal"];
const brotliAsync = promisify(brotliCompress);
const gzipAsync = promisify(gzip);

/**
 * Dateien unter `dir` (rekursiv), ohne die Unterordner `skip` der ersten Ebene.
 * @param {string} dir
 * @param {string[]} skip
 * @returns {string[]}
 */
function filesBelow(dir, skip) {
  /** @type {string[]} */
  const out = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (!skip.includes(e.name)) out.push(...filesBelow(p, []));
    } else if (e.isFile()) {
      out.push(p);
    }
  }
  return out;
}

/**
 * Legt `<datei>.br` und `<datei>.gz` zu jeder komprimierbaren Datei unter `dir` an
 * (nur Dateien über 1 KiB, nur Varianten, die kleiner sind als die Datei; eine
 * ältere Variante wird vorher entfernt).
 * @param {string} dir
 * @param {{ skip?: string[] }} [options] Unterordner der ersten Ebene, die ausgelassen werden
 * @returns {Promise<{ files: number, raw: number, br: number, gz: number }>} Anzahl der Dateien über
 *   1 KiB und Byte-Summen roh bzw. wie ausgeliefert (Variante, sonst roh)
 */
export async function precompressDir(dir, { skip = [] } = {}) {
  const stats = { files: 0, raw: 0, br: 0, gz: 0 };
  const targets = filesBelow(dir, skip).filter((f) => PRECOMPRESS_FILE.test(f));
  await Promise.all(
    targets.map(async (file) => {
      for (const ext of [".br", ".gz"]) rmSync(file + ext, { force: true });
      const data = readFileSync(file);
      if (data.length <= PRECOMPRESS_MIN_BYTES) return;
      const [br, gz] = await Promise.all([
        brotliAsync(data, {
          params: {
            [zlibConstants.BROTLI_PARAM_MODE]: zlibConstants.BROTLI_MODE_TEXT,
            [zlibConstants.BROTLI_PARAM_QUALITY]: zlibConstants.BROTLI_MAX_QUALITY,
            [zlibConstants.BROTLI_PARAM_SIZE_HINT]: data.length,
          },
        }),
        gzipAsync(data, { level: zlibConstants.Z_BEST_COMPRESSION }),
      ]);
      stats.files += 1;
      stats.raw += data.length;
      stats.br += Math.min(br.length, data.length);
      stats.gz += Math.min(gz.length, data.length);
      if (br.length < data.length) writeFileSync(file + ".br", br);
      if (gz.length < data.length) writeFileSync(file + ".gz", gz);
    }),
  );
  return stats;
}

/**
 * Vite-Plugin: komprimiert `build/` nach dem Adapter vor (ohne npm-Abhängigkeit).
 * Muss in `plugins` nach `sveltekit()` stehen (`buildApp` „post“ nach dem Adapter).
 * @returns {import("vite").Plugin}
 */
export function precompress() {
  let root = process.cwd();
  return {
    name: "klein-buch-precompress",
    apply: "build",
    configResolved(config) {
      root = config.root;
    },
    buildApp: {
      order: "post",
      async handler() {
        const s = await precompressDir(path.join(root, "build"), { skip: PRECOMPRESS_SKIP_DIRS });
        const kb = (/** @type {number} */ n) => `${Math.round(n / 1024)} KiB`;
        console.info(
          `klein-buch-precompress: ${s.files} Dateien, ${kb(s.raw)} → br ${kb(s.br)}, gzip ${kb(s.gz)}`,
        );
      },
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    // SvelteKit-Konfiguration (seit SvelteKit 3 hier statt in svelte.config.js):
    // Die SPA wird statisch gebaut und vom Rust-Server ausgeliefert (ADR 0039),
    // adapter-static mit Fallback auf index.html (SPA-Modus, kein SSR), Ausgabe
    // nach build/ wie bisher. Siehe https://svelte.dev/docs/kit/single-page-apps
    sveltekit({
      preprocess: vitePreprocess(),
      adapter: adapter({ fallback: "index.html" }),
      // Kein Zeitgeber für _app/version.json (Standard ab SvelteKit 3: stündlich).
      // SvelteKit prüft weiter selbst bei Fokus/Sichtbarkeit und nach Ladefehlern
      // (Neuladen nach einem Image-Update; ADR 0039 Nr. 140).
      version: { pollInterval: 0 },
    }),
    jsLicenses(),
    precompress(),
  ],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": { target: api },
      "/files": { target: api },
      "/healthz": { target: api },
      "/legal": { target: api },
    },
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
