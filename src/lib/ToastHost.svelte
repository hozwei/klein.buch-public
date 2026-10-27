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
  // - U7 (R3-02, 2.4.11): `--toast-h` ist der Abstand vom unteren Fensterrand bis
  //   zur Oberkante des Stapels (Höhe + Abstand unten + Safe-Area), nicht nur die
  //   Höhe. Das Layout verlängert damit auch den Inhalt unten (`padding-bottom`), so
  //   dass sich die Hauptaktion am Formularende über einen stehenden Toast scrollen
  //   lässt. Escape in einem Toast schließt ihn.
  // - U7 (R3-04, 2.4.3): Lag der Fokus im Toast, führt Schließen oder Aktion ihn
  //   weiter: zum nächsten Toast, sonst zum Element, das beim Erscheinen fokussiert
  //   war, sonst zur Seitenüberschrift bzw. zu <main> — nie auf <body>.
  import { tick } from "svelte";
  import {
    toastStore,
    dismissToast,
    pauseToast,
    resumeToast,
    toastOrigin,
    type ToastItem,
    type ToastKind,
  } from "#lib/toast.svelte.ts";
  import { pageFocusFallback } from "#lib/focusAfterRemoval.ts";

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
    const measure = () => {
      // Belegter Streifen unten: vom Fensterrand bis zur Oberkante des Stapels.
      const h = el.querySelector(".toast") ? Math.ceil(window.innerHeight - el.getBoundingClientRect().top) : 0;
      root.style.setProperty("--toast-h", h > 0 ? `${h + 4}px` : "0px");
    };
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
      root.style.removeProperty("--toast-h");
    };
  });

  function leave(e: FocusEvent, id: number) {
    const next = e.relatedTarget;
    if (next instanceof Node && (e.currentTarget as HTMLElement).contains(next)) return;
    resumeToast(id);
  }

  function visible(el: HTMLElement | null): el is HTMLElement {
    return !!el && el.isConnected && el.getClientRects().length > 0 && !el.closest("[inert]");
  }

  /** Fokus-Ziel nach dem Schließen von `t`, solange der Toast noch im DOM ist. */
  function nextFocusTarget(t: ToastItem): HTMLElement | null {
    const own = stack?.querySelector<HTMLElement>(`[data-toast-id="${t.id}"]`);
    const all = stack ? Array.from(stack.querySelectorAll<HTMLElement>(".toast")) : [];
    const i = own ? all.indexOf(own) : -1;
    const rest = i === -1 ? all : [...all.slice(i + 1), ...all.slice(0, i).reverse()];
    return rest[0]?.querySelector<HTMLElement>("button.close") ?? null;
  }

  async function close(t: ToastItem, withAction: boolean) {
    const hadFocus = !!stack?.querySelector(`[data-toast-id="${t.id}"]`)?.contains(document.activeElement);
    const nextToast = hadFocus ? nextFocusTarget(t) : null;
    const origin = toastOrigin(t.id);
    dismissToast(t.id);
    if (withAction) t.action?.run();
    if (!hadFocus) return;
    await tick();
    // Hat die Aktion den Fokus selbst gesetzt (z. B. Navigation, Dialog), bleibt er dort.
    const active = document.activeElement;
    if (active && active !== document.body && active.isConnected) return;
    const target = visible(nextToast) ? nextToast : visible(origin) ? origin : pageFocusFallback();
    target?.focus({ preventScroll: target !== origin });
  }

  function onKeydown(e: KeyboardEvent, t: ToastItem) {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    e.preventDefault();
    e.stopPropagation();
    void close(t, false);
  }
</script>

{#snippet toast(t: ToastItem)}
  <div
    class="toast {t.kind}"
    data-toast-id={t.id}
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
      <button type="button" class="btn-secondary action" onclick={() => close(t, true)} onkeydown={(e) => onKeydown(e, t)}>{t.action.label}</button>
    {/if}
    <button
      type="button"
      class="close"
      aria-label={CLOSE_NAME[t.kind]}
      onclick={() => close(t, false)}
      onkeydown={(e) => onKeydown(e, t)}
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
