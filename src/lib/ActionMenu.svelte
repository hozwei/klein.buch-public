<script lang="ts" module>
  /** Eintrag im Menü „Mehr“ (ActionMenu, PageBar `secondary`/`more`). */
  export interface ActionMenuItem {
    /** Sichtbare Beschriftung: Verb des Ergebnisses („PDF herunterladen“). */
    label: string;
    /** Aktion. Das Menü schließt vorher und gibt den Fokus an den Menü-Knopf zurück
     *  (ein Dialog, den die Aktion öffnet, kehrt dorthin zurück). */
    onSelect?: () => void;
    /** Navigation (Link). Mit `onSelect` zusammen: erst `onSelect`, dann der Link. */
    href?: string;
    /** Download-Link (`download`-Attribut; `true` = Dateiname vom Server). */
    download?: boolean | string;
    /** Gefahraktion (Stornieren, Verwerfen, Archivieren): steht unten, abgesetzt. */
    danger?: boolean;
    /** Gesperrt: bleibt im Menü erreichbar (aria-disabled), ohne Wirkung. */
    disabled?: boolean;
    /** Sichtbare Zusatzzeile, z. B. der Grund für „gesperrt“ (aria-describedby). */
    hint?: string;
  }
</script>

<script lang="ts">
  // Menü „Mehr“ (U4, U1-059/U3-036) nach dem Muster „Menu Button“ der WAI-ARIA APG:
  // - Knopf mit `aria-haspopup="menu"`, `aria-expanded`, `aria-controls`.
  //   Enter/Leertaste/Pfeil runter öffnen und fokussieren den ersten Eintrag,
  //   Pfeil hoch den letzten.
  // - Im Menü: Pfeiltasten (rundum), Pos1/Ende, Anfangsbuchstaben; Enter/Leertaste
  //   lösen aus; Escape schließt und gibt den Fokus an den Knopf zurück; Tab schließt
  //   und geht vom Knopf aus weiter. Klick daneben schließt.
  // - Gefahraktionen (`danger`) stehen unten hinter einer Trennlinie.
  // - Trefferflächen: Knopf --control-h, Einträge ≥ 36 px (im Drawer-Band 44 px).
  //
  // Das Menü liegt per Popover-API in der obersten Ebene (`popover="manual"`):
  // Scroll-Container (Tabellen) schneiden es nicht ab, und `<main>` (Container,
  // eigener Bezugsrahmen) verschiebt es nicht. Die Lage rechnet `place()` aus dem
  // Knopf (unter dem Knopf, sonst darüber; nie aus dem Fenster). Ohne Popover-API
  // (ältere Browser) hängt es absolut unter dem Knopf.
  import { tick } from "svelte";
  import { randomId } from "#lib/browser.ts";

  let {
    items,
    label = "Mehr",
    ariaLabel,
    variant = "secondary",
    align = "end",
    id,
  }: {
    items: ActionMenuItem[];
    /** Sichtbarer Text des Knopfs. */
    label?: string;
    /** Zugänglicher Name, wenn mehrere Menüs auf der Seite stehen (muss `label`
     *  enthalten, z. B. „Mehr zu RE-2026-0042“). */
    ariaLabel?: string;
    /** `ghost` in Tabellenzeilen neben einer sichtbaren Aktion, sonst `secondary`. */
    variant?: "secondary" | "ghost";
    /** Menü rechtsbündig unter dem Knopf (`end`, Standard) oder linksbündig. */
    align?: "end" | "start";
    id?: string;
  } = $props();

  const uid = randomId();
  const btnId = $derived(id ?? `am-${uid}`);
  const menuId = `am-menu-${uid}`;
  const supportsPopover =
    typeof HTMLElement !== "undefined" && typeof HTMLElement.prototype.showPopover === "function";

  let open = $state(false);
  let up = $state(false);
  let btn = $state<HTMLButtonElement | null>(null);
  let menu = $state<HTMLDivElement | null>(null);

  const regular = $derived(items.filter((i) => !i.danger));
  const dangerous = $derived(items.filter((i) => i.danger));

  function entries(): HTMLElement[] {
    return menu ? Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]')) : [];
  }

  async function openMenu(focus: "first" | "last" = "first") {
    if (open || items.length === 0) return;
    open = true;
    await tick();
    if (!menu) return;
    if (supportsPopover) {
      try {
        menu.showPopover();
      } catch {
        // schon offen oder nicht verbunden — Lage trotzdem rechnen
      }
    }
    place();
    const els = entries();
    (focus === "last" ? els[els.length - 1] : els[0])?.focus();
  }

  function closeMenu(returnFocus: boolean) {
    if (!open) return;
    open = false;
    if (supportsPopover && menu?.matches(":popover-open")) {
      try {
        menu.hidePopover();
      } catch {
        // bereits geschlossen
      }
    }
    if (returnFocus) btn?.focus();
  }

  function toggle() {
    if (open) closeMenu(true);
    else void openMenu("first");
  }

  /** Lage unter (sonst über) dem Knopf, innerhalb des Fensters. */
  function place() {
    if (!menu || !btn) return;
    const b = btn.getBoundingClientRect();
    const vh = window.innerHeight;
    // Knopf aus dem Bild gescrollt: Menü schließen statt es am Rand kleben zu lassen.
    if (b.bottom < 0 || b.top > vh) {
      closeMenu(false);
      return;
    }
    const m = menu.getBoundingClientRect();
    const gap = 4;
    const pad = 8;
    if (!supportsPopover) {
      up = vh - b.bottom < m.height + gap + pad && b.top > vh - b.bottom;
      return;
    }
    const vw = document.documentElement.clientWidth;
    let top = b.bottom + gap;
    if (top + m.height > vh - pad && b.top - gap - m.height >= pad) top = b.top - gap - m.height;
    top = Math.max(pad, Math.min(top, vh - pad - m.height));
    let left = align === "end" ? b.right - m.width : b.left;
    left = Math.max(pad, Math.min(left, vw - pad - m.width));
    menu.style.top = `${Math.round(top)}px`;
    menu.style.left = `${Math.round(left)}px`;
  }

  // Solange offen: Klick daneben schließt, Lage folgt Scrollen und Größenänderung,
  // Escape schließt vor einer umgebenden Fokusfalle (Fenster, Capture-Phase).
  $effect(() => {
    if (!open) return;
    let frame = 0;
    const onDown = (e: PointerEvent) => {
      const t = e.target;
      if (t instanceof Node && (menu?.contains(t) || btn?.contains(t))) return;
      closeMenu(false);
    };
    const onReflow = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        place();
      });
    };
    const onEscape = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      closeMenu(!!menu?.contains(document.activeElement) || document.activeElement === btn);
    };
    document.addEventListener("pointerdown", onDown, true);
    window.addEventListener("keydown", onEscape, true);
    window.addEventListener("resize", onReflow);
    window.addEventListener("scroll", onReflow, true);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("keydown", onEscape, true);
      window.removeEventListener("resize", onReflow);
      window.removeEventListener("scroll", onReflow, true);
      if (frame) cancelAnimationFrame(frame);
    };
  });

  function onButtonKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      void openMenu("first");
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      void openMenu("last");
    }
  }

  // Anfangsbuchstaben: springt zum nächsten Eintrag, der so beginnt.
  let typed = "";
  let typedAt = 0;
  const fold = (s: string) => s.toLocaleLowerCase("de");
  function typeahead(ch: string) {
    const now = Date.now();
    typed = now - typedAt > 700 ? ch : typed + ch;
    typedAt = now;
    const els = entries();
    const at = els.indexOf(document.activeElement as HTMLElement);
    const want = fold(typed);
    for (let k = typed.length > 1 ? 0 : 1; k <= els.length; k++) {
      const el = els[(at + k + els.length) % els.length];
      if (fold(el.dataset.label ?? "").startsWith(want)) {
        el.focus();
        return;
      }
    }
  }

  function onMenuKey(e: KeyboardEvent) {
    const els = entries();
    if (els.length === 0) return;
    const at = els.indexOf(document.activeElement as HTMLElement);
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        els[(at + 1) % els.length]?.focus();
        break;
      case "ArrowUp":
        e.preventDefault();
        els[(at - 1 + els.length) % els.length]?.focus();
        break;
      case "Home":
      case "PageUp":
        e.preventDefault();
        els[0]?.focus();
        break;
      case "End":
      case "PageDown":
        e.preventDefault();
        els[els.length - 1]?.focus();
        break;
      case "Tab":
        // Vom Knopf aus weiter (das Menü steht im DOM direkt hinter ihm). Hat eine
        // umgebende Fokusfalle den Fokus schon versetzt, nur schließen.
        closeMenu(!e.defaultPrevented);
        break;
      case " ":
        // Leertaste löst auch Links aus (APG); Knöpfe machen das selbst.
        if (document.activeElement instanceof HTMLAnchorElement) {
          e.preventDefault();
          document.activeElement.click();
        }
        break;
      default:
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && e.key !== " ") {
          typeahead(e.key);
        }
    }
  }

  function onMenuFocusOut(e: FocusEvent) {
    const next = e.relatedTarget;
    if (next instanceof Node && !menu?.contains(next) && !btn?.contains(next)) closeMenu(false);
  }

  function select(item: ActionMenuItem, e: MouseEvent) {
    if (item.disabled) {
      e.preventDefault();
      return;
    }
    // Links navigieren (der Fokus gehört dann der neuen Seite), alles andere kehrt
    // zum Knopf zurück.
    const navigates = !!item.href && !item.download;
    closeMenu(!navigates);
    item.onSelect?.();
  }
</script>

{#snippet entry(item: ActionMenuItem, n: number)}
  {#if item.href && !item.disabled}
    <a
      role="menuitem"
      tabindex="-1"
      class="kb-menu-item"
      class:danger={item.danger}
      href={item.href}
      download={item.download === true ? "" : item.download || undefined}
      data-label={item.label}
      aria-describedby={item.hint ? `${menuId}-h${n}` : undefined}
      onclick={(e) => select(item, e)}
    >
      <span class="kb-menu-label">{item.label}</span>
      {#if item.hint}<span id={`${menuId}-h${n}`} class="kb-menu-hint" aria-hidden="true">{item.hint}</span>{/if}
    </a>
  {:else}
    <button
      type="button"
      role="menuitem"
      tabindex="-1"
      class="kb-menu-item"
      class:danger={item.danger}
      aria-disabled={item.disabled ? "true" : undefined}
      data-label={item.label}
      aria-describedby={item.hint ? `${menuId}-h${n}` : undefined}
      onclick={(e) => select(item, e)}
    >
      <span class="kb-menu-label">{item.label}</span>
      {#if item.hint}<span id={`${menuId}-h${n}`} class="kb-menu-hint" aria-hidden="true">{item.hint}</span>{/if}
    </button>
  {/if}
{/snippet}

<div class="kb-menu" class:open>
  <button
    bind:this={btn}
    type="button"
    id={btnId}
    class={variant === "ghost" ? "btn-ghost kb-menu-btn" : "btn-secondary kb-menu-btn"}
    aria-haspopup="menu"
    aria-expanded={open}
    aria-controls={menuId}
    aria-label={ariaLabel}
    disabled={items.length === 0}
    onclick={toggle}
    onkeydown={onButtonKey}
  >
    <span>{label}</span>
    <svg
      class="kb-menu-chevron"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"><polyline points="4 6 8 10 12 6" /></svg
    >
  </button>
  <div
    bind:this={menu}
    id={menuId}
    role="menu"
    aria-labelledby={btnId}
    popover={supportsPopover ? "manual" : undefined}
    hidden={!supportsPopover && !open}
    class="kb-menu-list"
    class:fallback={!supportsPopover}
    class:up
    class:start={align === "start"}
    tabindex="-1"
    onkeydown={onMenuKey}
    onfocusout={onMenuFocusOut}
  >
    {#each regular as item, n}{@render entry(item, n)}{/each}
    {#if regular.length > 0 && dangerous.length > 0}<div role="separator" class="kb-menu-sep"></div>{/if}
    {#each dangerous as item, n}{@render entry(item, regular.length + n)}{/each}
  </div>
</div>

<style>
  .kb-menu {
    position: relative;
    display: inline-flex;
    flex: none;
  }
  .kb-menu-chevron {
    width: 16px;
    height: 16px;
    flex: none;
    transition: transform var(--t-fast) var(--ease-apple);
  }
  .kb-menu.open .kb-menu-chevron {
    transform: rotate(180deg);
  }
  /* Kein `display` setzen: sonst überstimmt die Seite das `display: none` des
     Browsers für geschlossene Popover bzw. `[hidden]`. `position: fixed` setzt der
     Browser für Popover selbst; die Lage (top/left) rechnet place() innerhalb des
     Fensters, der Knopf liegt ohnehin im Safe-Area-Innenabstand. */
  .kb-menu-list {
    inset: auto;
    margin: 0;
    box-sizing: border-box;
    min-width: 12rem;
    max-width: min(22rem, calc(100vw - 16px));
    max-height: calc(100vh - 16px);
    max-height: calc(100dvh - 16px);
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: var(--sp-1);
    background: var(--surface-card);
    color: var(--text);
    border: var(--rule-control);
    border-radius: var(--radius-control);
    box-shadow: var(--shadow-overlay);
    font-size: var(--type-body);
    text-align: left;
  }
  .kb-menu-list:focus {
    outline: none;
  }
  /* Rückfall ohne Popover-API: absolut am Knopf. */
  .kb-menu-list.fallback {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 60;
  }
  .kb-menu-list.fallback.start {
    right: auto;
    left: 0;
  }
  .kb-menu-list.fallback.up {
    top: auto;
    bottom: calc(100% + 4px);
  }
  .kb-menu-item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 2px;
    box-sizing: border-box;
    width: 100%;
    min-height: max(var(--target), 2.25rem);
    padding: var(--sp-1) var(--sp-3);
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-weight: 500;
    line-height: var(--lh-snug);
    text-align: left;
    text-decoration: none;
    cursor: pointer;
  }
  .kb-menu-item:focus {
    background: var(--surface-hover);
    outline: none;
  }
  .kb-menu-item:focus-visible {
    outline: var(--focus-width) solid var(--focus-ring);
    outline-offset: var(--focus-offset-inset);
  }
  @media (hover: hover) {
    .kb-menu-item:hover {
      background: var(--surface-hover);
      text-decoration: none;
    }
    .kb-menu-item.danger:hover {
      background: var(--status-danger-bg);
    }
  }
  .kb-menu-item.danger {
    color: var(--status-danger-fg);
  }
  .kb-menu-item.danger:focus {
    background: var(--status-danger-bg);
  }
  .kb-menu-item[aria-disabled="true"] {
    color: var(--text-muted);
    cursor: not-allowed;
  }
  .kb-menu-hint {
    font-size: var(--type-small);
    font-weight: 400;
    color: var(--text-muted);
  }
  .kb-menu-sep {
    height: 1px;
    margin: var(--sp-1) calc(-1 * var(--sp-1));
    background: var(--line);
  }
  @media (prefers-reduced-motion: reduce) {
    .kb-menu-chevron {
      transition: none;
    }
  }
  @media print {
    .kb-menu {
      display: none;
    }
  }
</style>
