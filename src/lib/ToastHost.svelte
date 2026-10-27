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
  // - Hover/Fokus hält die Uhr an (2.2.1); Fehler-, Warn- und Aktions-Toasts bleiben.
  // - U4: Art „Warnung“ (stehend, in der `status`-Region); jede Art trägt ein
  //   Symbol (Haken, Ausrufezeichen im Dreieck, Kreuz im Kreis), damit sie nicht nur
  //   an der Farbe erkennbar ist (1.4.1); Screenreader hören die Art über den
  //   Gruppennamen („Hinweis“, „Warnung“, „Fehler“).
  // - U4 (V4-02): Die beiden Regionen sind auch leer im Accessibility-Tree (keine
  //   Regel `display: none` mehr für leere Regionen — sonst entstünden Region und
  //   Meldung gleichzeitig, und höfliche Meldungen gingen je nach Screenreader
  //   verloren). Leer haben sie die Höhe 0; Abstand nur zwischen gefüllten Regionen.
  //   Art „info“ (neutral, z. B. abgebrochene Bestätigung) mit „i“-Symbol; der
  //   Schließen-Knopf nennt die Art („Fehlermeldung schließen“).
  import {
    toastStore,
    dismissToast,
    pauseToast,
    resumeToast,
    type ToastItem,
    type ToastKind,
  } from "#lib/toast.svelte.ts";

  const KIND_NAME: Record<ToastKind, string> = {
    ok: "Hinweis",
    info: "Hinweis",
    warning: "Warnung",
    error: "Fehler",
  };
  const CLOSE_NAME: Record<ToastKind, string> = {
    ok: "Hinweis schließen",
    info: "Hinweis schließen",
    warning: "Warnung schließen",
    error: "Fehlermeldung schließen",
  };

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
    aria-label={KIND_NAME[t.kind]}
    onmouseenter={() => pauseToast(t.id)}
    onmouseleave={() => resumeToast(t.id)}
    onfocusin={() => pauseToast(t.id)}
    onfocusout={(e) => leave(e, t.id)}
  >
    <svg class="icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      {#if t.kind === "error"}<circle cx="10" cy="10" r="7.5" /><path d="M7.2 7.2l5.6 5.6M12.8 7.2l-5.6 5.6" />
      {:else if t.kind === "warning"}<path d="M10 2.8 18 17H2z" /><path d="M10 8v4" /><path d="M10 14.6v.1" />
      {:else if t.kind === "info"}<circle cx="10" cy="10" r="7.5" /><path d="M10 9v4.6" /><path d="M10 6.4v.1" />
      {:else}<circle cx="10" cy="10" r="7.5" /><path d="M6.6 10.2l2.4 2.4 4.4-4.8" />{/if}
    </svg>
    <span class="msg">{t.message}</span>
    {#if t.action}
      <!-- V3-21: Aktion als normaler Sekundärknopf (Höhe --control-h, Kante wie alle Knöpfe). -->
      <button type="button" class="btn-secondary action" onclick={() => runAction(t)}>{t.action.label}</button>
    {/if}
    <button
      type="button"
      class="close"
      aria-label={CLOSE_NAME[t.kind]}
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
    gap: 0;
    width: min(28rem, calc(100vw - 2 * var(--sp-5)));
    /* Stapel bleibt durchklickbar; einzelne Toasts sind klickbar. */
    pointer-events: none;
  }
  .region {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  /* Leere Regionen bleiben im Accessibility-Tree (Höhe 0, V4-02); Abstand nur
     zwischen zwei gefüllten Regionen. */
  .region:has(.toast) + .region:has(.toast) { margin-top: var(--sp-2); }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-2) var(--sp-2) var(--sp-3);
    border: 1px solid;
    border-left-width: 4px;
    border-radius: var(--radius-control);
    box-shadow: var(--shadow-overlay);
    font-size: var(--type-body);
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
  .toast.warning {
    background: var(--status-warning-bg);
    color: var(--status-warning-fg);
    border-color: var(--status-warning-line);
  }
  .toast.info {
    background: var(--status-info-bg);
    color: var(--status-info-fg);
    border-color: var(--status-info-line);
  }
  .toast.error {
    background: var(--status-danger-bg);
    color: var(--status-danger-fg);
    border-color: var(--status-danger-line);
  }
  .toast .icon {
    flex: none;
    width: 20px;
    height: 20px;
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
  }
  @media (hover: hover) {
    .toast .close:hover { background: color-mix(in srgb, currentColor 10%, transparent); }
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
