import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

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
// SvelteKit baut zuerst den Server (SSR) und darin den Client; der Adapter
// leert `build/` erst danach. Deshalb schreibt der Client-Build die Liste
// zuerst nach `.svelte-kit/`, und das Ende des SSR-Builds kopiert sie nach
// dem Adapter an ihr Ziel. Der SSR-Build selbst zählt nicht: er wird nicht
// ausgeliefert.
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
 * Vite/Rollup-Plugin, das `build/legal/js-licenses.json` schreibt (ohne weitere npm-Abhängigkeit).
 * @returns {import("vite").Plugin}
 */
export function jsLicenses() {
  let root = process.cwd();
  let ssr = false;
  /** @param {string} file @param {JsLicense[]} list */
  const writeJson = (file, list) => {
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, JSON.stringify(list, null, 2) + "\n");
  };
  return {
    name: "klein-buch-js-licenses",
    apply: "build",
    configResolved(config) {
      root = config.root;
      ssr = Boolean(config.build.ssr);
    },
    buildStart() {
      if (ssr) rmSync(path.join(root, JS_LICENSES_STAGE), { force: true });
    },
    generateBundle(_options, bundle) {
      if (ssr) return;
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
      writeJson(path.join(root, JS_LICENSES_STAGE), list);
      // Ohne SvelteKit (kein SSR-Build danach) gilt diese Datei direkt.
      writeJson(path.join(root, JS_LICENSES_OUT), list);
    },
    closeBundle: {
      order: "post",
      sequential: true,
      handler() {
        if (!ssr) return;
        const stage = path.join(root, JS_LICENSES_STAGE);
        if (!existsSync(stage)) this.error(`${JS_LICENSES_STAGE} fehlt — der Client-Build hat keine Lizenzliste geschrieben.`);
        const out = path.join(root, JS_LICENSES_OUT);
        mkdirSync(path.dirname(out), { recursive: true });
        copyFileSync(stage, out);
      },
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [sveltekit(), jsLicenses()],
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
