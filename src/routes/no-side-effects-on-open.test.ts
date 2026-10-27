// Lauf F (ADR 0040, Zielmodell §9.2 Nr. 8): Seiten lösen beim Öffnen keine
// fachlichen Läufe und keine Schreib-Commands aus — manuelle Auslöser sind
// Knöpfe. Statische Prüfung der Quelltexte: Was beim Öffnen läuft, darf kein
// schreibender Command aus `#lib/api.ts` sein (W14-030 Abo-Rechnungen, D-14
// Zahlungskonten).
//
// U7 (R4-08): Geprüft werden alle `.svelte`-Dateien unter `src/routes` und `src/lib`
// (auch Bausteine wie `InvoiceEditor.svelte`), als Wurzeln `onMount`, `$effect`,
// `$effect.pre` und `afterNavigate`, dazu `load` in `+page.ts`/`+layout.ts`; lokale
// Funktionen der Datei werden bis drei Ebenen tief mitgelesen. Schreibend sind die
// Schreib-Commands aus `api.ts` sowie `uploadFile` und die schreibenden Auth-Aufrufe.

import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROUTES = path.dirname(fileURLToPath(import.meta.url));
const LIB = path.join(ROUTES, "..", "lib");
const API = path.join(LIB, "api.ts");
// Schreibende Aufrufe außerhalb der Commands (transport.ts): Upload und Auth.
const OTHER_WRITERS = [
  "uploadFile",
  "authLogin",
  "authLogout",
  "authLock",
  "authReauth",
  "authSetupOffer",
  "authSetupFinish",
  "authTotpRenewOffer",
  "authTotpRenewFinish",
  "authRecoveryRenew",
  "authChangePassphrase",
  "authRevokeOtherSessions",
];

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

function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) out.push(...sourceFiles(p));
    else if (name.endsWith(".svelte") || /^\+(page|layout)\.ts$/.test(name)) out.push(p);
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

const ROOTS = ["onMount(", "$effect(", "$effect.pre(", "afterNavigate("];
const LOAD = /export\s+(?:async\s+)?function\s+load\s*\(|export\s+const\s+load\b[^=]*=/g;
const LEVELS = 3;

/** Körper einer lokalen Funktion `name` (Deklaration oder `const name = (…) =>`). */
function localBody(src: string, name: string): string | null {
  const decl = new RegExp(`(?:async\\s+)?function\\s+${name}\\s*(?:<[^>]*>)?\\(`).exec(src);
  const arrow = new RegExp(`(?:const|let)\\s+${name}\\s*=\\s*(?:async\\s*)?\\(`).exec(src);
  const def = decl ?? arrow;
  if (!def) return null;
  const brace = src.indexOf("{", def.index + def[0].length - 1);
  return brace >= 0 ? balanced(src, brace) : null;
}

/**
 * Was beim Öffnen läuft: die Wurzeln (`onMount`, `$effect`, `$effect.pre`,
 * `afterNavigate`, `load`) plus Körper der darin aufgerufenen lokalen Funktionen,
 * bis `LEVELS` Ebenen tief.
 */
function onOpenCode(src: string): string {
  const parts: string[] = [];
  for (const root of ROOTS) {
    let at = src.indexOf(root);
    while (at >= 0) {
      parts.push(balanced(src, at + root.length - 1));
      at = src.indexOf(root, at + 1);
    }
  }
  for (const m of src.matchAll(LOAD)) {
    // Körper nach der Parameterliste (`function load(…) {…}` bzw. `(…) => {…}`).
    const open = src.indexOf("(", m.index);
    if (open < 0) continue;
    const brace = src.indexOf("{", open + balanced(src, open).length);
    if (brace >= 0) parts.push(balanced(src, brace));
  }
  const seen = new Set<string>();
  let frontier = parts.join("\n");
  for (let level = 0; level < LEVELS && frontier; level++) {
    const next: string[] = [];
    for (const m of frontier.matchAll(/\b([A-Za-z_]\w*)\s*\(/g)) {
      const name = m[1];
      if (seen.has(name)) continue;
      seen.add(name);
      const body = localBody(src, name);
      if (body) next.push(body);
    }
    parts.push(...next);
    frontier = next.join("\n");
  }
  return parts.join("\n");
}

describe("Keine Seiteneffekte beim Öffnen einer Seite", () => {
  const commands = apiCommands();
  const writers = [
    ...[...commands.entries()].filter(([, cmd]) => !isRead(cmd)).map(([fn]) => fn),
    ...OTHER_WRITERS,
  ];

  it("erkennt die Schreib-Commands aus api.ts", () => {
    expect(commands.get("recurringInvoicesRunDueCheck")).toBe("recurring_invoices_run_due_check");
    expect(writers).toContain("recurringInvoicesRunDueCheck");
    expect(writers).toContain("recurringRunDueCheck");
    expect(writers).not.toContain("recurringInvoicesList");
    expect(writers).not.toContain("paymentAccountsList");
  });

  it("deckt Bausteine, $effect und load ab (R4-08)", () => {
    const files = [...sourceFiles(ROUTES), ...sourceFiles(LIB)].map((f) => path.basename(f));
    expect(files).toContain("InvoiceEditor.svelte");
    expect(files).toContain("ToastHost.svelte");
    expect(files).toContain("+page.ts");
    const probe = `
      let x = $state(0);
      $effect(() => { if (x) step(); });
      function step() { inner(); }
      async function inner() { await contactsCreate({}); }
    `;
    expect(onOpenCode(probe)).toContain("contactsCreate(");
    const load = `export function load({ params }) { return helper(params); }
      function helper(p) { return uploadFile("beleg", p); }`;
    expect(onOpenCode(load)).toContain("uploadFile(");
    expect(writers).toContain("contactsCreate");
  });

  for (const file of [...sourceFiles(ROUTES), ...sourceFiles(LIB)]) {
    const rel = path.relative(path.join(ROUTES, ".."), file);
    it(`${rel}: Öffnen ruft keinen Schreib-Command auf`, () => {
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
