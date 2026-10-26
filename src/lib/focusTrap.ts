// Fokusfalle für Dialoge und den Navigations-Drawer (Lauf 2026.11, U1).
//
// Als Svelte-Action: `<div role="dialog" use:focusTrap={{ onEscape: close }}>`.
//
// - Beim Öffnen wandert der Fokus in den Dialog: auf das Element mit
//   `data-initial-focus`, sonst auf `initialFocus` (Selektor), sonst auf das
//   erste fokussierbare Element, sonst auf den Dialog selbst.
// - Tab und Umschalt+Tab bleiben im Dialog (Ringverkehr).
// - Escape ruft `onEscape` auf (ohne `onEscape` bleibt der Dialog offen, z. B.
//   die Anmelde-Maske).
// - Fokus, der den Dialog trotzdem verlässt (Klick daneben, Skript), wird
//   zurückgeholt.
// - Beim Schließen kehrt der Fokus zum Auslöser zurück, sofern niemand
//   anderes ihn inzwischen bekommen hat. Der Rücksprung läuft leicht
//   verzögert, weil das Layout `inert` erst nach dem Schließen vom
//   Hintergrund nimmt (modalStack) — ein inertes Element nimmt keinen Fokus.
// - Solange eine Falle aktiv ist, scrollt das Dokument nicht (Hintergrund
//   bleibt stehen, auf dem Handy scrollt nur der Dialog).
//
// Gestapelte Fallen (Bestätigung über dem Über-Dialog): nur die oberste
// reagiert auf Tasten und Fokuswechsel.
//
// Bewusst ohne DOM-Bibliothek; die reinen Teile (`nextTrapTarget`,
// `TABBABLE_SELECTOR`) sind in focusTrap.test.ts ohne Browser geprüft.

export interface FocusTrapOptions {
  /** Escape gedrückt (nur oberste Falle). Ohne Angabe: Escape bewirkt nichts. */
  onEscape?: () => void;
  /** Selektor für den Startfokus; `[data-initial-focus]` hat Vorrang. */
  initialFocus?: string;
  /** Fokus beim Schließen zum Auslöser zurück (Standard: ja). */
  returnFocus?: boolean;
  /**
   * Ausdrückliches Rücksprungziel statt des beim Öffnen fokussierten Elements.
   * Nötig, wenn der Auslöser beim Öffnen `inert` wird (Menü-Knopf des
   * Drawers): Chromium nimmt ihm dann den Fokus, bevor die Falle ihn sieht,
   * und der gemerkte Auslöser wäre `<body>`.
   */
  returnTo?: HTMLElement | null;
  /** Dokument-Scrollen sperren (Standard: ja). */
  lockScroll?: boolean;
}

export const TABBABLE_SELECTOR = [
  "a[href]",
  "area[href]",
  "button:not([disabled])",
  'input:not([disabled]):not([type="hidden"])',
  "select:not([disabled])",
  "textarea:not([disabled])",
  "iframe",
  "audio[controls]",
  "video[controls]",
  "summary",
  '[contenteditable]:not([contenteditable="false"])',
  "[tabindex]",
].join(",");

/**
 * Ziel des nächsten Tab-Schritts innerhalb der Falle oder `null`, wenn der
 * Browser selbst weitergehen darf.
 *
 * @param items  fokussierbare Elemente in DOM-Reihenfolge
 * @param active aktuell fokussiertes Element (oder `null`)
 * @param shift  Umschalt+Tab
 * @param inside liegt `active` innerhalb der Falle?
 */
export function nextTrapTarget<T>(
  items: readonly T[],
  active: T | null,
  shift: boolean,
  inside: boolean,
): T | null {
  if (items.length === 0) return null;
  const first = items[0];
  const last = items[items.length - 1];
  if (!inside || active === null) return shift ? last : first;
  const i = items.indexOf(active);
  if (i === -1) return shift ? last : first;
  if (shift && i === 0) return last;
  if (!shift && i === items.length - 1) return first;
  return null;
}

/** Fokussierbare (per Tab erreichbare) Elemente in `root`, DOM-Reihenfolge. */
export function tabbables(root: HTMLElement): HTMLElement[] {
  const out: HTMLElement[] = [];
  for (const el of Array.from(root.querySelectorAll<HTMLElement>(TABBABLE_SELECTOR))) {
    if (el.tabIndex < 0) continue;
    if (el.closest("[inert],[hidden]")) continue;
    const cv = (el as HTMLElement & { checkVisibility?: (o?: object) => boolean }).checkVisibility;
    if (typeof cv === "function" && !cv.call(el, { visibilityProperty: true })) continue;
    out.push(el);
  }
  return out;
}

// ---- Stapel und Scroll-Sperre (modulweit) ----------------------------------

type Trap = { node: HTMLElement; opts: FocusTrapOptions };
const stack: Trap[] = [];
let scrollLocks = 0;
let savedOverflow = "";

function lockScroll(): void {
  if (scrollLocks++ === 0) {
    const de = document.documentElement;
    savedOverflow = de.style.overflow;
    de.style.overflow = "hidden";
  }
}
function unlockScroll(): void {
  scrollLocks = Math.max(0, scrollLocks - 1);
  if (scrollLocks === 0) document.documentElement.style.overflow = savedOverflow;
}

function isTop(t: Trap): boolean {
  return stack[stack.length - 1] === t;
}

function focusInitial(t: Trap): void {
  const { node, opts } = t;
  const pick =
    node.querySelector<HTMLElement>("[data-initial-focus]") ??
    (opts.initialFocus ? node.querySelector<HTMLElement>(opts.initialFocus) : null) ??
    tabbables(node)[0] ??
    null;
  if (pick) {
    pick.focus();
    return;
  }
  if (!node.hasAttribute("tabindex")) node.setAttribute("tabindex", "-1");
  node.focus();
}

/** Fokus zurückgeben, sobald der Auslöser wieder fokussierbar ist. */
function restoreFocus(el: HTMLElement | null, from?: HTMLElement): void {
  if (!el) return;
  let tries = 0;
  const attempt = () => {
    if (!el.isConnected) return;
    const a = document.activeElement;
    // Jemand anderes hat den Fokus bewusst gesetzt → nicht stehlen. Fokus, der
    // noch in der geschlossenen Falle steht (WebKit nimmt ihn inerten bzw.
    // versteckten Elementen nicht immer ab), zählt nicht als bewusst gesetzt.
    const stale = !!a && !!from && from.contains(a);
    if (a && !stale && a !== document.body && a !== document.documentElement && a !== el) return;
    el.focus();
    if (document.activeElement !== el && ++tries < 5) setTimeout(attempt, 16 * tries);
  };
  setTimeout(attempt, 0);
}

/** Svelte-Action: Fokusfalle (siehe Kopfkommentar). */
export function focusTrap(node: HTMLElement, options: FocusTrapOptions = {}) {
  const trap: Trap = { node, opts: options };
  // <body>/<html> sind kein Auslöser (Fokus war nirgends oder ging beim Öffnen verloren).
  const ae = document.activeElement;
  const trigger =
    ae instanceof HTMLElement && ae !== document.body && ae !== document.documentElement && !node.contains(ae)
      ? ae
      : null;

  function onKeydown(e: KeyboardEvent): void {
    if (!isTop(trap)) return;
    if (e.key === "Escape") {
      if (trap.opts.onEscape) {
        e.preventDefault();
        e.stopPropagation();
        trap.opts.onEscape();
      }
      return;
    }
    if (e.key !== "Tab") return;
    const items = tabbables(node);
    const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (items.length === 0) {
      e.preventDefault();
      node.focus();
      return;
    }
    const target = nextTrapTarget(items, active, e.shiftKey, !!active && node.contains(active));
    if (target) {
      e.preventDefault();
      target.focus();
    }
  }

  function onFocusIn(e: FocusEvent): void {
    if (!isTop(trap)) return;
    const t = e.target;
    if (t instanceof Node && !node.contains(t)) focusInitial(trap);
  }

  stack.push(trap);
  if (options.lockScroll !== false) lockScroll();
  document.addEventListener("keydown", onKeydown, true);
  document.addEventListener("focusin", onFocusIn);
  // Erst nach dem Rendern des Inhalts fokussieren.
  queueMicrotask(() => {
    if (node.isConnected && isTop(trap) && !node.contains(document.activeElement)) focusInitial(trap);
  });

  return {
    update(next: FocusTrapOptions = {}) {
      trap.opts = next;
    },
    destroy() {
      document.removeEventListener("keydown", onKeydown, true);
      document.removeEventListener("focusin", onFocusIn);
      const i = stack.indexOf(trap);
      if (i !== -1) stack.splice(i, 1);
      if (trap.opts.lockScroll !== false) unlockScroll();
      if (trap.opts.returnFocus !== false) restoreFocus(trap.opts.returnTo ?? trigger, node);
    },
  };
}
