// G2-DOC.3.1 — Kategorien-Whitelist des User-Handbuchs.
//
// Single-Source-of-Truth für (a) die zulässigen Werte des Front-Matter-Feldes
// `category` und (b) die Reihenfolge + deutschen Anzeige-Labels im
// `/help`-Sidebar-Kapitelbaum.
//
// Spiegelt 1:1 die 7 Werte aus `src-tauri/resources/handbook/README.md`. Der
// Rust-seitige Front-Matter-Verify-Test (`handbook_resources_test`) bewacht die
// Eingangsseite (Markdown). Der TS-Index-Loader (`./index.ts`) bewacht die
// Ausgangsseite (Frontend).

export const HANDBOOK_CATEGORIES = [
  "erste-schritte",
  "bedienen",
  "recht-und-steuern",
  "faq",
  "troubleshooting",
  "glossar",
  // W13: Betreiber-Kapitel (Installation, Proxy, Volumes, Updates) am Ende,
  // getrennt von den Kapiteln für die tägliche Bedienung.
  "betrieb",
] as const;

export type HandbookCategory = (typeof HANDBOOK_CATEGORIES)[number];

/** Deutsche Sidebar-Labels. Reihenfolge = Anzeigereihenfolge im Kapitelbaum. */
export const HANDBOOK_CATEGORY_LABELS: Record<HandbookCategory, string> = {
  "erste-schritte": "Erste Schritte",
  bedienen: "Bedienen",
  "recht-und-steuern": "Recht und Steuern",
  faq: "Häufige Fragen",
  troubleshooting: "Fehlerbehebung",
  glossar: "Glossar",
  betrieb: "Betrieb",
};

export function isHandbookCategory(s: string): s is HandbookCategory {
  return (HANDBOOK_CATEGORIES as readonly string[]).includes(s);
}
