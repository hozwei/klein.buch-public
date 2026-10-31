// ADR 0042 Nr. 16 (V3-12, V3-18, V3-10, V2-21): statische Prüfung der Quelltexte, dass die
// abgelösten Klassen nicht zurückkommen und die Bausteine der Kartenansicht vorhanden sind
// (design.md §4.5, §4.8, §4.10). Die Gestaltung selbst prüft der UI-Audit.

import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...sourceFiles(p));
    else if (/\.(svelte|css)$/.test(e.name)) out.push(p);
  }
  return out;
}

const FILES = sourceFiles(SRC).map((p) => ({ rel: path.relative(SRC, p), text: readFileSync(p, "utf8") }));
const read = (rel: string) => readFileSync(path.join(SRC, rel), "utf8");

/** Klassennamen aus `class="…"` und `class:name` (ohne Kommentare). */
function classNames(text: string): Set<string> {
  const code = text.replace(/<!--[\s\S]*?-->/g, "").replace(/\/\*[\s\S]*?\*\//g, "");
  const names = new Set<string>();
  for (const m of code.matchAll(/\bclass="([^"]*)"/g)) {
    for (const n of m[1].replace(/\{[^}]*\}/g, " ").split(/\s+/)) if (n) names.add(n);
  }
  for (const m of code.matchAll(/\bclass:([\w-]+)/g)) names.add(m[1]);
  return names;
}

/** CSS-Selektoren, die eine Klasse verwenden (ohne Kommentare; in Svelte nur `<style>`). */
function cssUses(f: { rel: string; text: string }, cls: string): boolean {
  const code = f.text.replace(/\/\*[\s\S]*?\*\//g, "");
  const styles = f.rel.endsWith(".css") ? code : (code.match(/<style[\s\S]*?<\/style>/g) ?? []).join("\n");
  return new RegExp(`\\.${cls.replace(/-/g, "\\-")}(?![\\w-])`).test(styles);
}

describe("Abgelöste Klassen (ADR 0042 Nr. 16)", () => {
  const RETIRED = [
    // V3-18: Hinweisflächen nur über Banner
    "caveat", "info-banner", "danger-banner", "due-banner", "edit-hint",
    // V3-10: Beträge nur über .kb-num
    "num", "right",
    // V2-21: Links nur über .kb-link
    "linklike", "txt-link", "inline-link", "tgt",
  ];

  it.each(RETIRED)("Klasse .%s wird weder vergeben noch gestylt", (cls) => {
    const hits = FILES.filter((f) => classNames(f.text).has(cls) || cssUses(f, cls)).map((f) => f.rel);
    expect(hits, `.${cls} noch in`).toEqual([]);
  });

  it("tokens.css definiert die Altklassen der Hinweisflächen nicht mehr", () => {
    const css = read("lib/styles/tokens.css").replace(/\/\*[\s\S]*?\*\//g, "");
    for (const cls of ["caveat", "info-banner", "danger-banner"]) {
      expect(css, `.${cls}`).not.toMatch(new RegExp(`\\.${cls}\\b`));
    }
  });
});

describe("Hierarchie der Kartenansicht (V3-12)", () => {
  it("tokens.css hat die Bausteinklassen", () => {
    const css = read("lib/styles/tokens.css");
    for (const cls of ["kb-card-head", "kb-card-status", "kb-card-figure", "kb-card-omit"]) {
      expect(css, cls).toContain(`td.${cls}`);
    }
  });

  // Listen mit Detailseite: Kopf, Status bzw. Hauptbetrag gesetzt.
  const LISTS: [string, string[]][] = [
    ["routes/invoices/+page.svelte", ["kb-card-head", "kb-card-status", "kb-card-figure"]],
    ["routes/quotes/+page.svelte", ["kb-card-head", "kb-card-status", "kb-card-figure"]],
    ["routes/expenses/+page.svelte", ["kb-card-head", "kb-card-status", "kb-card-figure"]],
    ["routes/assets/+page.svelte", ["kb-card-head", "kb-card-status", "kb-card-figure"]],
    ["routes/private-movements/+page.svelte", ["kb-card-head", "kb-card-status", "kb-card-figure"]],
    ["routes/contacts/+page.svelte", ["kb-card-head", "kb-card-omit"]],
    ["routes/packages/+page.svelte", ["kb-card-head", "kb-card-status"]],
    ["routes/settings/payment-accounts/+page.svelte", ["kb-card-head", "kb-card-status"]],
    ["routes/recurring-invoices/+page.svelte", ["kb-card-head"]],
    ["routes/expenses/recurring/+page.svelte", ["kb-card-head", "kb-card-figure"]],
    ["routes/settings/backup-log/+page.svelte", ["kb-card-head", "kb-card-status"]],
    ["routes/settings/mail-log/+page.svelte", ["kb-card-head", "kb-card-status"]],
  ];

  it.each(LISTS)("%s", (rel, classes) => {
    const names = classNames(read(rel));
    for (const cls of classes) expect(names.has(cls), cls).toBe(true);
  });
});

describe("Versand-Historie ohne Sekunden (ADR 0041 Nr. 18)", () => {
  it.each(["routes/invoices/[id]/+page.svelte", "routes/quotes/[id]/+page.svelte"])("%s", (rel) => {
    const text = read(rel);
    const block = text.slice(text.indexOf("Versand-Historie"), text.indexOf("Vollständiges Protokoll"));
    expect(block).toContain("dateTimeShort(e.createdAt)");
    expect(block).not.toContain("dateTime(e.createdAt)");
  });
});

describe("Links im Text nur über .kb-link (V2-21, ADR 0041 Nr. 31)", () => {
  const SVELTE = FILES.filter((f) => f.rel.endsWith(".svelte"));

  // Seiteneigene Regeln wie `.crossref a { text-decoration: underline }`. Ausgenommen sind
  // `:global(a)` für fremdes Markup (Handbuch-Prosa aus Markdown, Links im Banner).
  it("kein seiteneigenes Unterstreichen von <a>", () => {
    const hits: string[] = [];
    for (const f of SVELTE) {
      const styles = (f.text.replace(/\/\*[\s\S]*?\*\//g, "").match(/<style[\s\S]*?<\/style>/g) ?? []).join("\n");
      for (const m of styles.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
        const selector = m[1].trim();
        if (selector.includes(":global(")) continue;
        if (/(^|[\s>+~,])a(?=[\s:.,[]|$)/.test(selector) && /text-decoration(-line)?:\s*underline/.test(m[2])) {
          hits.push(`${f.rel}: ${selector}`);
        }
      }
    }
    expect(hits).toEqual([]);
  });

  // Ein <a> ohne Klasse in einem Absatz ist ein Link im Fließtext: ohne .kb-link nur über
  // die Farbe erkennbar (WCAG 1.4.1). Navigation und Listen stehen nicht in <p>.
  it("<a> in einem Absatz trägt eine Klasse", () => {
    const hits: string[] = [];
    for (const f of SVELTE) {
      const code = f.text.replace(/<!--[\s\S]*?-->/g, "");
      for (const p of code.matchAll(/<p\b[^>]*>([\s\S]*?)<\/p>/g)) {
        for (const a of p[1].matchAll(/<a\b([^>]*)>/g)) {
          if (!/\bclass=/.test(a[1])) hits.push(`${f.rel}: <a${a[1]}>`);
        }
      }
    }
    expect(hits).toEqual([]);
  });
});

describe("Ansage der Banner (V6-R04)", () => {
  it("Banner leitet die Rolle aus `live` ab", () => {
    const text = read("lib/Banner.svelte");
    expect(text).toContain('mode === "assertive" ? "alert" : mode === "polite" ? "status" : "note"');
  });

  it("„Ergebnis unklar“ beim Passphrase-Wechsel wird sofort angesagt", () => {
    expect(read("routes/settings/security/+page.svelte")).toMatch(
      /<Banner kind="warning" live="assertive" title="Ergebnis unklar\.">/,
    );
  });

  it("Steuer-Disclaimer ohne Live-Region, mit Namen", () => {
    expect(read("routes/help/[slug]/+page.svelte")).toContain(
      '<Banner kind="warning" live="off" label="Steuer-Disclaimer">',
    );
  });

  it("Restore-Entwürfe mit Knöpfen stehen nicht im Banner", () => {
    const text = read("routes/settings/backup/+page.svelte");
    const start = text.indexOf('<p class="drafts-text">');
    const bannerEnd = text.indexOf("</Banner>", start);
    expect(text.indexOf('<ul class="drafts">', start)).toBeGreaterThan(bannerEnd);
  });
});

describe("Keine Statusfarbe für Nicht-Status (V6-R06, V3-07/08)", () => {
  it("Automatik zur Jahreswende ohne Info-Fläche", () => {
    expect(read("routes/fiscal-year/+page.svelte")).not.toMatch(/--status-info-(bg|line)/);
  });
});

describe("Zeitformat der Hinweise (V6-R05)", () => {
  it("Verworfen-Hinweis der Rechnung ohne Sekunden", () => {
    const text = read("routes/invoices/[id]/+page.svelte");
    expect(text).toContain("Am {dateTimeShort(inv.voidedAt)} verworfen");
  });

  it("Status der Backup-Seite ohne Sekunden, Verlauf mit Sekunden", () => {
    const text = read("routes/settings/backup/+page.svelte");
    const status = text.slice(text.indexOf('<dl class="bk-status">'), text.indexOf("</dl>"));
    expect(status).not.toMatch(/\bdateTime\(/);
    expect(text).toContain('data-label="Erstellt">{dateTime(item.createdAt)}');
  });
});
