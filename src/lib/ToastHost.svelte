<script lang="ts">
  // Globaler Toast-Host. Genau einmal im Root-Layout gemountet; rendert den Stack
  // aus dem zentralen toastStore. Aufgerufen wird ausschließlich über flash()
  // aus #lib/toast.svelte.ts — Seiten mounten keinen eigenen Toast.
  //
  // Dateiname bewusst ToastHost (nicht Toast) — sonst Casing-Kollision mit dem
  // Store-Modul toast.svelte.ts auf case-insensitiven Dateisystemen (Windows).
  //
  // U1 (Lauf 2026.11):
  // - Position UNTEN (Handy: volle Breite über der Safe-Area, sonst rechts
  //   unten). Oben liegen Kopfleiste und PageBar mit den Hauptaktionen; dort
  //   würden Toasts genau diese verdecken. Die Höhe des Stapels geht als
  //   `--toast-h` in `scroll-padding-bottom`, damit fokussierte Felder nicht
  //   darunter verschwinden (2.4.11).
  // - Zwei dauerhaft vorhandene Live-Regionen (4.1.3): Fehler in
  //   `role="alert"`, alles andere in `role="status"` — neu eingefügte
  //   Regionen würden manche Screenreader nicht ansagen.
  // - Hover/Fokus hält die Uhr an (2.2.1); Fehler/Aktions-Toasts bleiben.
  import {
    toastStore,
    dismissToast,
    pauseToast,
    resumeToast,
    type ToastItem,
  } from "#lib/toast.svelte.ts";

  let stack = $state<HTMLDivElement | null>(null);
  const errors = $derived(toastStore.items.filter((t) => t.kind === "error"));
  const others = $derived(toastStore.items.filter((t) => t.kind !== "error"));

  $effect(() => {
    const el = stack;
    if (!el || typeof ResizeObserver === "undefined") return;
    const root = document.documentElement;
    const ro = new ResizeObserver(() => {
      const h = el.querySelector(".toast") ? Math.ceil(el.getBoundingClientRect().height) : 0;
      root.style.setProperty("--toast-h", h > 0 ? `${h + 12}px` : "0px");
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
      root.style.removeProperty("--toast-h");
    };
  });

  function leave(e: FocusEvent, id: number) {
    const next = e.relatedTarget;
    if (next instanceof Node && (e.currentTarget as HTMLElement).contains(next)) return;
    resumeToast(id);
  }

  function runAction(t: ToastItem) {
    dismissToast(t.id);
    t.action?.run();
  }
</script>

{#snippet toast(t: ToastItem)}
  <div
    class="toast {t.kind}"
    role="group"
    aria-label={t.kind === "error" ? "Fehler" : "Hinweis"}
    onmouseenter={() => pauseToast(t.id)}
    onmouseleave={() => resumeToast(t.id)}
    onfocusin={() => pauseToast(t.id)}
    onfocusout={(e) => leave(e, t.id)}
  >
    <span class="msg">{t.message}</span>
    {#if t.action}
      <button type="button" class="action" onclick={() => runAction(t)}>{t.action.label}</button>
    {/if}
    <button
      type="button"
      class="close"
      aria-label="Hinweis schließen"
      onclick={() => dismissToast(t.id)}
    ><span aria-hidden="true">×</span></button>
  </div>
{/snippet}

<div class="toast-stack" bind:this={stack}>
  <div class="region" role="alert" aria-live="assertive" aria-atomic="false">
    {#each errors as t (t.id)}{@render toast(t)}{/each}
  </div>
  <div class="region" role="status" aria-live="polite" aria-atomic="false">
    {#each others as t (t.id)}{@render toast(t)}{/each}
  </div>
</div>

<style>
  .toast-stack {
    position: fixed;
    right: var(--sp-5);
    bottom: calc(var(--sp-5) + env(safe-area-inset-bottom, 0px));
    z-index: 1500;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    width: min(28rem, calc(100vw - 2 * var(--sp-5)));
    /* Stapel bleibt durchklickbar; einzelne Toasts sind klickbar. */
    pointer-events: none;
  }
  .region {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .region:empty { display: none; }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-2) var(--sp-2) var(--sp-4);
    border: 1px solid;
    border-left-width: 4px;
    border-radius: var(--r-md);
    box-shadow: var(--sh-lg);
    font-size: var(--fs-base);
    line-height: 1.45;
    overflow-wrap: anywhere;
    animation: slide-in var(--t-base) var(--ease-apple);
  }
  .toast .msg {
    flex: 1;
    padding: var(--sp-1) 0;
  }
  .toast.ok {
    background: var(--status-success-bg);
    color: var(--status-success-fg);
    border-color: var(--status-success-line);
  }
  .toast.error {
    background: var(--status-danger-bg);
    color: var(--status-danger-fg);
    border-color: var(--status-danger-line);
  }
  .toast .close {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: max(var(--target), 32px);
    min-height: max(var(--target), 32px);
    padding: 0;
    background: transparent;
    border: 0;
    border-radius: var(--r-sm);
    font-size: 1.25rem;
    line-height: 1;
    color: inherit;
    cursor: pointer;
  }
  .toast .action {
    flex: none;
    min-height: max(var(--target), 32px);
    padding: 0 var(--sp-3);
    background: var(--surface-card);
    color: inherit;
    border: 1px solid currentColor;
    border-radius: var(--r-sm);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  @media (hover: hover) {
    .toast .close:hover { background: rgba(16, 36, 44, 0.08); }
  }
  @keyframes slide-in {
    from { transform: translateY(8px); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }
  /* Handy: unten über die ganze Breite, über der Safe-Area. */
  @media (max-width: 600px) {
    .toast-stack {
      left: calc(var(--sp-3) + env(safe-area-inset-left, 0px));
      right: calc(var(--sp-3) + env(safe-area-inset-right, 0px));
      bottom: calc(var(--sp-3) + env(safe-area-inset-bottom, 0px));
      width: auto;
    }
  }
  @media print {
    .toast-stack { display: none; }
  }
</style>
