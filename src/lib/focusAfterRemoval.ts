// Fokus nach dem Ausblenden einer Tabellenzeile (U7, R3-04; design.md §4.1, ADR 0041
// Nr. 33: nie auf <body>). Blendet eine Aktion die Zeile aus (Konto deaktivieren, Abo
// pausieren bei ausgeblendeten pausierten Einträgen), verschwindet der fokussierte
// Knopf. Wie `focusAfterDismiss` der Hinweise-Liste: erste Aktion der Zeile, die jetzt
// an derselben Stelle steht (sonst der vorigen), sonst das Ersatzziel, sonst die
// Seitenüberschrift bzw. <main>. Hat etwas anderes den Fokus übernommen (Dialog,
// Navigation), bleibt er dort.

/** Seitenüberschrift (programmatisch fokussierbar) bzw. <main> als letztes Ziel. */
export function pageFocusFallback(): HTMLElement | null {
  if (typeof document === "undefined") return null;
  const main = document.getElementById("main-content");
  const h1 = main?.querySelector<HTMLElement>("h1") ?? null;
  if (h1 && h1.getClientRects().length > 0) {
    if (!h1.hasAttribute("tabindex")) h1.setAttribute("tabindex", "-1");
    return h1;
  }
  return main;
}

export function focusLost(): boolean {
  if (typeof document === "undefined") return false;
  const a = document.activeElement;
  return !a || a === document.body || !a.isConnected;
}

/**
 * @param index Position der ausgeblendeten Zeile vor dem Neuladen.
 * @param fallback Ersatzziel ohne passende Zeile (z. B. Filter-Kasten).
 */
export function focusRowAfterRemoval(index: number, fallback?: HTMLElement | null): void {
  if (!focusLost()) return;
  const rows = Array.from(document.querySelectorAll<HTMLElement>("#main-content tbody tr"));
  const at = Math.max(0, Math.min(index, rows.length));
  const order = [...rows.slice(at), ...rows.slice(0, at).reverse()];
  for (const row of order) {
    const target = row.querySelector<HTMLElement>("a[href], button:not([disabled])");
    if (target && target.getClientRects().length > 0) {
      target.focus();
      return;
    }
  }
  const fb = fallback && fallback.isConnected ? fallback : pageFocusFallback();
  fb?.focus();
}
