// Lauf F (ADR 0040, Zielmodell §9.2 Nr. 8): Seiten lösen beim Öffnen keine
// fachlichen Läufe und keine Schreib-Commands aus — manuelle Auslöser sind
// Knöpfe. Statische Prüfung der Quelltexte: Was `onMount` (eine Ebene tief über
// gleichnamige Funktionen der Datei) aufruft, darf kein schreibender Command
// aus `$lib/api` sein (W14-030 Abo-Rechnungen, D-14 Zahlungskonten).

import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROUTES = path.dirname(fileURLToPath(import.meta.url));
const API = path.join(ROUTES, "..", "lib", "api.ts");

// Lesende Commands wie in `commands::registry::kind_of` (Suffixe + Liste).
const READ_SUFFIXES = [
  "_list",
  "_get",
  "_search",
  "_data",
  "_info",
  "_overview",
  "_history",
  "_preview",
  "_count",
  "_status",
  "_check",
];
const READ_NAMES = new Set([
  "euer_compute_report",
  "euer_available_years",
  "euer_package",
  "euer_afa_pending",
  "assets_afa_table",
  "assets_suggest_method",
  "depreciation_list_for_year",
  "email_log_for",
  "invoices_validate_draft",
  "quotes_validate_draft",
  "quotes_legal_bindings",
  "travel_compute",
  "expenses_receipt_xml_text",
  // liest nur (Registry führt es nach der Standardregel als „Write“)
  "backup_get_settings",
]);
// Ausnahmen von der Suffix-Regel: Läufe, die Belege erzeugen.
const WRITE_NAMES = new Set(["recurring_run_due_check", "recurring_invoices_run_due_check"]);

function isRead(command: string): boolean {
  if (WRITE_NAMES.has(command)) return false;
  return READ_NAMES.has(command) || READ_SUFFIXES.some((s) => command.endsWith(s));
}

/** api.ts: exportierte Funktion → Command-Name (erster `invoke`-Aufruf im Körper). */
function apiCommands(): Map<string, string> {
  const src = readFileSync(API, "utf8");
  const out = new Map<string, string>();
  const re = /export (?:async )?function (\w+)\s*(?:<[^>]*>)?\(/g;
  let m: RegExpExecArray | null;
  const starts: Array<{ name: string; at: number }> = [];
  while ((m = re.exec(src))) starts.push({ name: m[1], at: m.index });
  starts.forEach((s, i) => {
    const body = src.slice(s.at, i + 1 < starts.length ? starts[i + 1].at : src.length);
    const call = /invoke(?:<[^(]*>)?\(\s*"([a-z0-9_]+)"/.exec(body);
    if (call) out.set(s.name, call[1]);
  });
  return out;
}

function svelteFiles(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) out.push(...svelteFiles(p));
    else if (name === "+page.svelte" || name === "+layout.svelte") out.push(p);
  }
  return out;
}

/** Text einer Klammer ab `open` (Index von `(` bzw. `{`) bis zur passenden Schließung. */
function balanced(src: string, open: number): string {
  const pairs: Record<string, string> = { "(": ")", "{": "}" };
  const close = pairs[src[open]];
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    if (src[i] === src[open]) depth++;
    else if (src[i] === close && --depth === 0) return src.slice(open, i + 1);
  }
  return src.slice(open);
}

/** Was beim Öffnen läuft: `onMount(...)` plus Körper der darin aufgerufenen lokalen Funktionen. */
function onOpenCode(src: string): string {
  const parts: string[] = [];
  let at = src.indexOf("onMount(");
  while (at >= 0) {
    parts.push(balanced(src, at + "onMount".length));
    at = src.indexOf("onMount(", at + 1);
  }
  const direct = parts.join("\n");
  const locals = new Set<string>();
  for (const m of direct.matchAll(/\b(\w+)\b/g)) locals.add(m[1]);
  for (const name of locals) {
    const def = new RegExp(`(?:async\\s+)?function\\s+${name}\\s*\\(`).exec(src);
    if (!def) continue;
    const brace = src.indexOf("{", def.index + def[0].length - 1);
    if (brace >= 0) parts.push(balanced(src, brace));
  }
  return parts.join("\n");
}

describe("Keine Seiteneffekte beim Öffnen einer Seite", () => {
  const commands = apiCommands();
  const writers = [...commands.entries()].filter(([, cmd]) => !isRead(cmd)).map(([fn]) => fn);

  it("erkennt die Schreib-Commands aus api.ts", () => {
    expect(commands.get("recurringInvoicesRunDueCheck")).toBe("recurring_invoices_run_due_check");
    expect(writers).toContain("recurringInvoicesRunDueCheck");
    expect(writers).toContain("recurringRunDueCheck");
    expect(writers).not.toContain("recurringInvoicesList");
    expect(writers).not.toContain("paymentAccountsList");
  });

  for (const file of svelteFiles(ROUTES)) {
    const rel = path.relative(ROUTES, file);
    it(`${rel}: onMount ruft keinen Schreib-Command auf`, () => {
      const code = onOpenCode(readFileSync(file, "utf8"));
      const hits = writers.filter((fn) => new RegExp(`\\b${fn}\\s*\\(`).test(code));
      expect(hits, `${rel} schreibt beim Öffnen: ${hits.join(", ")}`).toEqual([]);
    });
  }

  it("Abo-Rechnungen: Fälligkeitslauf nur über den Knopf (W14-030)", () => {
    const src = readFileSync(path.join(ROUTES, "recurring-invoices", "+page.svelte"), "utf8");
    expect(src).not.toContain("triggerDueOnOpen");
    expect(onOpenCode(src)).not.toContain("recurringInvoicesRunDueCheck");
    expect(src).toContain("recurringInvoicesRunDueCheck()");
  });

  it("Zahlungskonten: Seite liest nur (D-14)", () => {
    const src = readFileSync(
      path.join(ROUTES, "settings", "payment-accounts", "+page.svelte"),
      "utf8",
    );
    expect(src).not.toMatch(/EnsureDefaults/);
  });
});
